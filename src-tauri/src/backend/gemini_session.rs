use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;

use crate::backend::events::{AppServerEvent, EventSink};
use crate::types::WorkspaceEntry;

/// Represents a conversation thread for Gemini
#[derive(Clone, Debug)]
pub(crate) struct GeminiThread {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
    pub(crate) created_at: String,
    pub(crate) messages: Vec<GeminiMessage>,
}

#[derive(Clone, Debug)]
pub(crate) struct GeminiMessage {
    pub(crate) role: String, // "user" or "assistant"
    pub(crate) content: String,
}

/// Session that manages Gemini CLI interactions for a workspace
pub(crate) struct GeminiWorkspaceSession {
    pub(crate) entry: WorkspaceEntry,
    pub(crate) threads: Mutex<HashMap<String, GeminiThread>>,
    pub(crate) active_process: Mutex<Option<Child>>,
    pub(crate) next_id: AtomicU64,
    /// Callbacks for background threads
    pub(crate) background_thread_callbacks: Mutex<HashMap<String, mpsc::UnboundedSender<Value>>>,
}

impl GeminiWorkspaceSession {
    pub(crate) fn new(entry: WorkspaceEntry) -> Self {
        Self {
            entry,
            threads: Mutex::new(HashMap::new()),
            active_process: Mutex::new(None),
            next_id: AtomicU64::new(1),
            background_thread_callbacks: Mutex::new(HashMap::new()),
        }
    }

    /// Create a new thread
    pub(crate) async fn create_thread(&self) -> GeminiThread {
        let id = format!("thread-{}", uuid::Uuid::new_v4());
        let thread = GeminiThread {
            id: id.clone(),
            title: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            messages: Vec::new(),
        };
        self.threads.lock().await.insert(id, thread.clone());
        thread
    }

    /// Get a thread by ID
    pub(crate) async fn get_thread(&self, thread_id: &str) -> Option<GeminiThread> {
        self.threads.lock().await.get(thread_id).cloned()
    }

    /// List all threads
    pub(crate) async fn list_threads(&self) -> Vec<GeminiThread> {
        self.threads.lock().await.values().cloned().collect()
    }

    /// Archive (delete) a thread
    pub(crate) async fn archive_thread(&self, thread_id: &str) -> bool {
        self.threads.lock().await.remove(thread_id).is_some()
    }

    /// Add a message to a thread
    pub(crate) async fn add_message(&self, thread_id: &str, role: &str, content: &str) -> bool {
        let mut threads = self.threads.lock().await;
        if let Some(thread) = threads.get_mut(thread_id) {
            thread.messages.push(GeminiMessage {
                role: role.to_string(),
                content: content.to_string(),
            });
            true
        } else {
            false
        }
    }

    /// Build the conversation history as a prompt for Gemini
    pub(crate) async fn build_conversation_prompt(&self, thread_id: &str, new_message: &str) -> Option<String> {
        let threads = self.threads.lock().await;
        let thread = threads.get(thread_id)?;

        let mut prompt = String::new();

        // Include conversation history
        for msg in &thread.messages {
            match msg.role.as_str() {
                "user" => prompt.push_str(&format!("User: {}\n\n", msg.content)),
                "assistant" => prompt.push_str(&format!("Assistant: {}\n\n", msg.content)),
                _ => {}
            }
        }

        // Add the new message
        prompt.push_str(&format!("User: {}\n\nAssistant:", new_message));

        Some(prompt)
    }

    /// Interrupt the currently running process
    pub(crate) async fn interrupt(&self) -> bool {
        let mut active = self.active_process.lock().await;
        if let Some(ref mut child) = *active {
            let _ = child.kill().await;
            *active = None;
            true
        } else {
            false
        }
    }
}

/// Build PATH environment with common binary locations
pub(crate) fn build_gemini_path_env(gemini_bin: Option<&str>) -> Option<String> {
    let mut paths: Vec<String> = env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect();

    let mut extras = vec![
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ]
    .into_iter()
    .map(|value| value.to_string())
    .collect::<Vec<String>>();

    if let Ok(home) = env::var("HOME") {
        extras.push(format!("{home}/.local/bin"));
        extras.push(format!("{home}/.local/share/mise/shims"));
        extras.push(format!("{home}/.cargo/bin"));
        extras.push(format!("{home}/.bun/bin"));
        // Add Google Cloud SDK path for gemini
        extras.push(format!("{home}/google-cloud-sdk/bin"));
        let nvm_root = Path::new(&home).join(".nvm/versions/node");
        if let Ok(entries) = std::fs::read_dir(nvm_root) {
            for entry in entries.flatten() {
                let bin_path = entry.path().join("bin");
                if bin_path.is_dir() {
                    extras.push(bin_path.to_string_lossy().to_string());
                }
            }
        }
    }

    if let Some(bin_path) = gemini_bin.filter(|value| !value.trim().is_empty()) {
        let parent = Path::new(bin_path).parent();
        if let Some(parent) = parent {
            extras.push(parent.to_string_lossy().to_string());
        }
    }

    for extra in extras {
        if !paths.contains(&extra) {
            paths.push(extra);
        }
    }

    if paths.is_empty() {
        None
    } else {
        Some(paths.join(":"))
    }
}

/// Build a command for the Gemini CLI
pub(crate) fn build_gemini_command(gemini_bin: Option<String>) -> Command {
    let bin = gemini_bin
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "gemini".into());
    let mut command = Command::new(bin);
    if let Some(path_env) = build_gemini_path_env(gemini_bin.as_deref()) {
        command.env("PATH", path_env);
    }
    command
}

/// Check if Gemini CLI is installed
pub(crate) async fn check_gemini_installation(
    gemini_bin: Option<String>,
) -> Result<Option<String>, String> {
    let mut command = build_gemini_command(gemini_bin);
    command.arg("--version");
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let output = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        command.output(),
    )
    .await
    {
        Ok(result) => result.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "Gemini CLI not found. Install Gemini CLI and ensure `gemini` is on your PATH."
                    .to_string()
            } else {
                e.to_string()
            }
        })?,
        Err(_) => {
            return Err(
                "Timed out while checking Gemini CLI. Make sure `gemini --version` runs in Terminal."
                    .to_string(),
            );
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        if detail.is_empty() {
            return Err(
                "Gemini CLI failed to start. Try running `gemini --version` in Terminal."
                    .to_string(),
            );
        }
        return Err(format!(
            "Gemini CLI failed to start: {detail}. Try running `gemini --version` in Terminal."
        ));
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if version.is_empty() { None } else { Some(version) })
}

/// Spawn Gemini for a single turn and stream output
pub(crate) async fn spawn_gemini_turn<E: EventSink>(
    session: Arc<GeminiWorkspaceSession>,
    thread_id: String,
    turn_id: String,
    prompt: String,
    gemini_bin: Option<String>,
    event_sink: E,
) -> Result<(), String> {
    let workspace_id = session.entry.id.clone();

    // Build the Gemini command
    let mut command = build_gemini_command(gemini_bin);
    command.current_dir(&session.entry.path);
    command.arg("-p");
    command.arg(&prompt);
    command.arg("--output-format");
    command.arg("stream-json");
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("missing stdout")?;
    let stderr = child.stderr.take().ok_or("missing stderr")?;

    // Store the active process for potential interruption
    *session.active_process.lock().await = Some(child);

    // Emit turn/started event
    let payload = AppServerEvent {
        workspace_id: workspace_id.clone(),
        message: json!({
            "method": "turn/started",
            "params": {
                "threadId": thread_id,
                "turn": {
                    "id": turn_id,
                    "threadId": thread_id
                }
            }
        }),
    };
    event_sink.emit_app_server_event(payload);

    // Generate item ID for the assistant message
    let item_id = format!("item-{}", uuid::Uuid::new_v4());

    // Emit item/started for the agent message
    let payload = AppServerEvent {
        workspace_id: workspace_id.clone(),
        message: json!({
            "method": "item/started",
            "params": {
                "threadId": thread_id,
                "item": {
                    "id": item_id,
                    "type": "agentMessage",
                    "text": ""
                }
            }
        }),
    };
    event_sink.emit_app_server_event(payload);

    let session_clone = Arc::clone(&session);
    let workspace_id_clone = workspace_id.clone();
    let thread_id_clone = thread_id.clone();
    let turn_id_clone = turn_id.clone();
    let item_id_clone = item_id.clone();
    let event_sink_clone = event_sink.clone();

    // Spawn task to handle stdout (streaming JSON from Gemini)
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut full_response = String::new();

        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            // Try to parse as JSON (Gemini stream-json format)
            if let Ok(json_value) = serde_json::from_str::<Value>(&line) {
                // Handle different Gemini streaming event types
                if let Some(text) = json_value.get("text").and_then(|t| t.as_str()) {
                    full_response.push_str(text);

                    // Check for background thread callback
                    let mut sent_to_background = false;
                    {
                        let callbacks = session_clone.background_thread_callbacks.lock().await;
                        if let Some(tx) = callbacks.get(&thread_id_clone) {
                            let _ = tx.send(json!({
                                "method": "item/agentMessage/delta",
                                "params": {
                                    "threadId": thread_id_clone,
                                    "itemId": item_id_clone,
                                    "delta": text
                                }
                            }));
                            sent_to_background = true;
                        }
                    }

                    if !sent_to_background {
                        // Emit delta event
                        let payload = AppServerEvent {
                            workspace_id: workspace_id_clone.clone(),
                            message: json!({
                                "method": "item/agentMessage/delta",
                                "params": {
                                    "threadId": thread_id_clone,
                                    "itemId": item_id_clone,
                                    "delta": text
                                }
                            }),
                        };
                        event_sink_clone.emit_app_server_event(payload);
                    }
                } else if let Some(candidates) = json_value.get("candidates") {
                    // Handle candidate-based response format
                    if let Some(parts) = candidates
                        .get(0)
                        .and_then(|c| c.get("content"))
                        .and_then(|c| c.get("parts"))
                    {
                        if let Some(text) = parts.get(0).and_then(|p| p.get("text")).and_then(|t| t.as_str()) {
                            full_response.push_str(text);

                            let mut sent_to_background = false;
                            {
                                let callbacks = session_clone.background_thread_callbacks.lock().await;
                                if let Some(tx) = callbacks.get(&thread_id_clone) {
                                    let _ = tx.send(json!({
                                        "method": "item/agentMessage/delta",
                                        "params": {
                                            "threadId": thread_id_clone,
                                            "itemId": item_id_clone,
                                            "delta": text
                                        }
                                    }));
                                    sent_to_background = true;
                                }
                            }

                            if !sent_to_background {
                                let payload = AppServerEvent {
                                    workspace_id: workspace_id_clone.clone(),
                                    message: json!({
                                        "method": "item/agentMessage/delta",
                                        "params": {
                                            "threadId": thread_id_clone,
                                            "itemId": item_id_clone,
                                            "delta": text
                                        }
                                    }),
                                };
                                event_sink_clone.emit_app_server_event(payload);
                            }
                        }
                    }
                }
            } else {
                // If not JSON, treat as plain text response
                full_response.push_str(&line);
                full_response.push('\n');

                let mut sent_to_background = false;
                {
                    let callbacks = session_clone.background_thread_callbacks.lock().await;
                    if let Some(tx) = callbacks.get(&thread_id_clone) {
                        let _ = tx.send(json!({
                            "method": "item/agentMessage/delta",
                            "params": {
                                "threadId": thread_id_clone,
                                "itemId": item_id_clone,
                                "delta": format!("{}\n", line)
                            }
                        }));
                        sent_to_background = true;
                    }
                }

                if !sent_to_background {
                    let payload = AppServerEvent {
                        workspace_id: workspace_id_clone.clone(),
                        message: json!({
                            "method": "item/agentMessage/delta",
                            "params": {
                                "threadId": thread_id_clone,
                                "itemId": item_id_clone,
                                "delta": format!("{}\n", line)
                            }
                        }),
                    };
                    event_sink_clone.emit_app_server_event(payload);
                }
            }
        }

        // Add the response to the thread's message history
        session_clone.add_message(&thread_id_clone, "assistant", &full_response).await;

        // Emit item/completed event
        let mut sent_to_background = false;
        {
            let callbacks = session_clone.background_thread_callbacks.lock().await;
            if let Some(tx) = callbacks.get(&thread_id_clone) {
                let _ = tx.send(json!({
                    "method": "item/completed",
                    "params": {
                        "threadId": thread_id_clone,
                        "item": {
                            "id": item_id_clone,
                            "type": "agentMessage",
                            "text": full_response
                        }
                    }
                }));
                sent_to_background = true;
            }
        }

        if !sent_to_background {
            let payload = AppServerEvent {
                workspace_id: workspace_id_clone.clone(),
                message: json!({
                    "method": "item/completed",
                    "params": {
                        "threadId": thread_id_clone,
                        "item": {
                            "id": item_id_clone,
                            "type": "agentMessage",
                            "text": full_response
                        }
                    }
                }),
            };
            event_sink_clone.emit_app_server_event(payload);
        }

        // Emit turn/completed event
        {
            let callbacks = session_clone.background_thread_callbacks.lock().await;
            if let Some(tx) = callbacks.get(&thread_id_clone) {
                let _ = tx.send(json!({
                    "method": "turn/completed",
                    "params": {
                        "threadId": thread_id_clone,
                        "turn": {
                            "id": turn_id_clone,
                            "threadId": thread_id_clone
                        }
                    }
                }));
            } else {
                let payload = AppServerEvent {
                    workspace_id: workspace_id_clone.clone(),
                    message: json!({
                        "method": "turn/completed",
                        "params": {
                            "threadId": thread_id_clone,
                            "turn": {
                                "id": turn_id_clone,
                                "threadId": thread_id_clone
                            }
                        }
                    }),
                };
                event_sink_clone.emit_app_server_event(payload);
            }
        }

        // Clear the active process
        *session_clone.active_process.lock().await = None;
    });

    // Spawn task to handle stderr
    let workspace_id_stderr = workspace_id.clone();
    let event_sink_stderr = event_sink.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let payload = AppServerEvent {
                workspace_id: workspace_id_stderr.clone(),
                message: json!({
                    "method": "gemini/stderr",
                    "params": { "message": line },
                }),
            };
            event_sink_stderr.emit_app_server_event(payload);
        }
    });

    Ok(())
}

/// Create a new workspace session (no persistent process needed)
pub(crate) async fn create_workspace_session<E: EventSink>(
    entry: WorkspaceEntry,
    default_gemini_bin: Option<String>,
    client_version: String,
    event_sink: E,
) -> Result<Arc<GeminiWorkspaceSession>, String> {
    let gemini_bin = entry
        .gemini_bin
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or(default_gemini_bin);

    // Check that Gemini is installed
    let _ = check_gemini_installation(gemini_bin).await?;

    let session = Arc::new(GeminiWorkspaceSession::new(entry.clone()));

    // Emit connected event
    let payload = AppServerEvent {
        workspace_id: entry.id.clone(),
        message: json!({
            "method": "gemini/connected",
            "params": { "workspaceId": entry.id.clone() }
        }),
    };
    event_sink.emit_app_server_event(payload);

    Ok(session)
}
