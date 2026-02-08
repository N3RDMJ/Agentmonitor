use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;

use crate::backend::events::{AppServerEvent, EventSink};
use crate::codex::args::parse_codex_args;
use crate::shared::process_core::{kill_child_process_tree, tokio_command};
use crate::types::WorkspaceEntry;

#[cfg(target_os = "windows")]
use crate::shared::process_core::{build_cmd_c_command, resolve_windows_executable};

fn extract_thread_id(value: &Value) -> Option<String> {
    let params = value.get("params")?;

    params
        .get("threadId")
        .or_else(|| params.get("thread_id"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            params
                .get("thread")
                .and_then(|thread| thread.get("id"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        })
}

fn build_initialize_params(client_version: &str) -> Value {
    json!({
        "clientInfo": {
            "name": "codex_monitor",
            "title": "Agent Monitor",
            "version": client_version
        },
        "capabilities": {
            "experimentalApi": true
        }
    })
}

pub(crate) struct WorkspaceSession {
    pub(crate) entry: WorkspaceEntry,
    pub(crate) child: Mutex<Option<Child>>,
    pub(crate) stdin: Mutex<Option<ChildStdin>>,
    pub(crate) pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    pub(crate) next_id: AtomicU64,
    mode: SessionMode,
    compatible: Mutex<CompatibleSessionState>,
    app_event_emitter: Arc<dyn Fn(Value) + Send + Sync>,
    /// Callbacks for background threads - events for these threadIds are sent through the channel
    pub(crate) background_thread_callbacks: Mutex<HashMap<String, mpsc::UnboundedSender<Value>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionMode {
    JsonRpc,
    CompatiblePty,
}

#[derive(Clone, Debug)]
struct CompatibleThread {
    id: String,
    name: String,
    preview: String,
    cwd: String,
    created_at: u64,
    updated_at: u64,
    archived: bool,
}

struct CompatibleSessionState {
    cli_bin: String,
    cli_args: Vec<String>,
    threads: HashMap<String, CompatibleThread>,
    next_thread_seq: u64,
    next_turn_seq: u64,
    next_item_seq: u64,
    active_turn_interrupts: HashMap<String, Arc<AtomicBool>>,
}

impl CompatibleSessionState {
    fn new(cli_bin: String, cli_args: Vec<String>) -> Self {
        Self {
            cli_bin,
            cli_args,
            threads: HashMap::new(),
            next_thread_seq: 1,
            next_turn_seq: 1,
            next_item_seq: 1,
            active_turn_interrupts: HashMap::new(),
        }
    }
}

fn now_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn build_compatible_thread_value(thread: &CompatibleThread) -> Value {
    json!({
        "id": thread.id,
        "title": thread.name,
        "preview": thread.preview,
        "cwd": thread.cwd,
        "createdAt": thread.created_at,
        "updatedAt": thread.updated_at,
    })
}

fn extract_user_text_from_turn_input(params: &Value) -> Option<String> {
    let input = params.get("input")?.as_array()?;
    let mut text_parts: Vec<String> = Vec::new();
    for item in input {
        let item_type = item
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if item_type != "text" {
            continue;
        }
        let text = item
            .get("text")
            .and_then(|value| value.as_str())
            .map(|value| value.trim())
            .unwrap_or("");
        if !text.is_empty() {
            text_parts.push(text.to_string());
        }
    }
    if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join("\n\n"))
    }
}

fn build_compatible_cli_invocation(base_args: &[String], prompt: &str) -> (Vec<String>, bool) {
    let mut replaced = false;
    let args = base_args
        .iter()
        .map(|arg| {
            if arg.contains("{prompt}") {
                replaced = true;
                arg.replace("{prompt}", prompt)
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>();
    if replaced {
        (args, false)
    } else {
        (args, true)
    }
}

fn run_compatible_pty_command(
    cwd: String,
    cli_bin: String,
    cli_args: Vec<String>,
    prompt: String,
    use_stdin_prompt: bool,
    interrupt_signal: Arc<AtomicBool>,
) -> Result<String, String> {
    fn abort_pty_child(
        child: &mut Box<dyn portable_pty::Child + Send + Sync>,
        message: String,
    ) -> Result<String, String> {
        let _ = child.kill();
        let _ = child.wait();
        Err(message)
    }

    let pty_system = native_pty_system();
    let size = PtySize {
        rows: 40,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = pty_system
        .openpty(size)
        .map_err(|err| format!("Failed to open PTY sidecar: {err}"))?;
    let mut command = CommandBuilder::new(cli_bin);
    command.cwd(cwd);
    for arg in cli_args {
        command.arg(arg);
    }
    command.env("TERM", "xterm-256color");

    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|err| format!("Failed to spawn PTY sidecar process: {err}"))?;

    let mut writer = match pair.master.take_writer() {
        Ok(writer) => writer,
        Err(err) => {
            return abort_pty_child(
                &mut child,
                format!("Failed to open PTY sidecar writer: {err}"),
            );
        }
    };
    if use_stdin_prompt {
        if let Err(err) = writer.write_all(prompt.as_bytes()) {
            return abort_pty_child(
                &mut child,
                format!("Failed writing prompt to PTY sidecar: {err}"),
            );
        }
        if let Err(err) = writer.write_all(b"\n\x04") {
            return abort_pty_child(
                &mut child,
                format!("Failed finalizing prompt write to PTY sidecar: {err}"),
            );
        }
        if let Err(err) = writer.flush() {
            return abort_pty_child(
                &mut child,
                format!("Failed flushing PTY sidecar input: {err}"),
            );
        }
    }
    drop(writer);

    let mut reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(err) => {
            return abort_pty_child(
                &mut child,
                format!("Failed to open PTY sidecar reader: {err}"),
            );
        }
    };
    let mut output = String::new();
    let mut buffer = [0_u8; 4096];
    loop {
        if interrupt_signal.load(Ordering::SeqCst) {
            let _ = child.kill();
            break;
        }
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                output.push_str(&String::from_utf8_lossy(&buffer[..count]));
            }
            Err(err) => {
                return abort_pty_child(
                    &mut child,
                    format!("Failed reading PTY sidecar output: {err}"),
                );
            }
        }
    }

    let status = child
        .wait()
        .map_err(|err| format!("Failed waiting on PTY sidecar process: {err}"))?;
    if status.success() || interrupt_signal.load(Ordering::SeqCst) {
        return Ok(output);
    }

    let code = status.exit_code();
    let summary = output.trim();
    if summary.is_empty() {
        Err(format!("Compatible CLI exited with code {}", code))
    } else {
        Err(format!(
            "Compatible CLI exited with code {}: {}",
            code, summary
        ))
    }
}

impl WorkspaceSession {
    fn emit_app_message(&self, message: Value) {
        (self.app_event_emitter)(message);
    }

    pub(crate) async fn terminate_process(&self) {
        let child = { self.child.lock().await.take() };
        if let Some(mut child) = child {
            kill_child_process_tree(&mut child).await;
        }
    }

    async fn write_message(&self, value: Value) -> Result<(), String> {
        if self.mode != SessionMode::JsonRpc {
            return Ok(());
        }
        let mut stdin_guard = self.stdin.lock().await;
        let Some(stdin) = stdin_guard.as_mut() else {
            return Err("missing stdin".to_string());
        };
        let mut line = serde_json::to_string(&value).map_err(|e| e.to_string())?;
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())
    }

    async fn send_request_compatible(
        self: &Arc<Self>,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        match method {
            "thread/start" => {
                let mut state = self.compatible.lock().await;
                let thread_id = format!("compat-thread-{}", state.next_thread_seq);
                state.next_thread_seq += 1;
                let now = now_epoch_seconds();
                let thread = CompatibleThread {
                    id: thread_id.clone(),
                    name: "New Agent".to_string(),
                    preview: String::new(),
                    cwd: self.entry.path.clone(),
                    created_at: now,
                    updated_at: now,
                    archived: false,
                };
                state.threads.insert(thread_id.clone(), thread.clone());
                drop(state);
                self.emit_app_message(json!({
                    "method": "thread/started",
                    "params": {
                        "thread": build_compatible_thread_value(&thread),
                    }
                }));
                Ok(json!({
                    "result": {
                        "threadId": thread_id,
                        "thread": build_compatible_thread_value(&thread),
                    }
                }))
            }
            "thread/list" => {
                let cursor = params
                    .get("cursor")
                    .and_then(|value| value.as_str())
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let limit = params
                    .get("limit")
                    .and_then(|value| value.as_u64())
                    .map(|value| value.max(1) as usize)
                    .unwrap_or(20);
                let sort_key = params
                    .get("sortKey")
                    .and_then(|value| value.as_str())
                    .unwrap_or("updated_at");

                let state = self.compatible.lock().await;
                let mut threads = state
                    .threads
                    .values()
                    .filter(|thread| !thread.archived)
                    .cloned()
                    .collect::<Vec<_>>();
                if sort_key == "created_at" {
                    threads.sort_by(|a, b| {
                        b.created_at
                            .cmp(&a.created_at)
                            .then_with(|| a.id.cmp(&b.id))
                    });
                } else {
                    threads.sort_by(|a, b| {
                        b.updated_at
                            .cmp(&a.updated_at)
                            .then_with(|| a.id.cmp(&b.id))
                    });
                }
                let page = threads
                    .iter()
                    .skip(cursor)
                    .take(limit)
                    .map(build_compatible_thread_value)
                    .collect::<Vec<_>>();
                let next_cursor = if cursor + page.len() < threads.len() {
                    Some((cursor + page.len()).to_string())
                } else {
                    None
                };
                Ok(json!({
                    "result": {
                        "data": page,
                        "nextCursor": next_cursor,
                    }
                }))
            }
            "thread/resume" => {
                let thread_id = params
                    .get("threadId")
                    .or_else(|| params.get("thread_id"))
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| "threadId is required".to_string())?
                    .to_string();
                let state = self.compatible.lock().await;
                let thread = state
                    .threads
                    .get(&thread_id)
                    .ok_or_else(|| "thread not found".to_string())?;
                Ok(json!({
                    "result": {
                        "threadId": thread_id,
                        "thread": build_compatible_thread_value(thread),
                    }
                }))
            }
            "thread/fork" => {
                let source_thread_id = params
                    .get("threadId")
                    .or_else(|| params.get("thread_id"))
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| "threadId is required".to_string())?
                    .to_string();
                let mut state = self.compatible.lock().await;
                let source = state
                    .threads
                    .get(&source_thread_id)
                    .cloned()
                    .ok_or_else(|| "thread not found".to_string())?;
                let thread_id = format!("compat-thread-{}", state.next_thread_seq);
                state.next_thread_seq += 1;
                let now = now_epoch_seconds();
                let thread = CompatibleThread {
                    id: thread_id.clone(),
                    name: source.name,
                    preview: source.preview,
                    cwd: source.cwd,
                    created_at: now,
                    updated_at: now,
                    archived: false,
                };
                state.threads.insert(thread_id.clone(), thread.clone());
                drop(state);
                self.emit_app_message(json!({
                    "method": "thread/started",
                    "params": {
                        "thread": build_compatible_thread_value(&thread),
                    }
                }));
                Ok(json!({
                    "result": {
                        "threadId": thread_id,
                        "thread": build_compatible_thread_value(&thread),
                    }
                }))
            }
            "thread/archive" => {
                let thread_id = params
                    .get("threadId")
                    .or_else(|| params.get("thread_id"))
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| "threadId is required".to_string())?
                    .to_string();
                let mut state = self.compatible.lock().await;
                if let Some(thread) = state.threads.get_mut(&thread_id) {
                    thread.archived = true;
                }
                Ok(json!({ "result": { "ok": true } }))
            }
            "thread/name/set" => {
                let thread_id = params
                    .get("threadId")
                    .or_else(|| params.get("thread_id"))
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| "threadId is required".to_string())?
                    .to_string();
                let name = params
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "name is required".to_string())?;
                let mut state = self.compatible.lock().await;
                if let Some(thread) = state.threads.get_mut(&thread_id) {
                    thread.name = name.clone();
                    thread.updated_at = now_epoch_seconds();
                }
                drop(state);
                self.emit_app_message(json!({
                    "method": "thread/name/updated",
                    "params": {
                        "threadId": thread_id,
                        "threadName": name,
                    }
                }));
                Ok(json!({ "result": { "ok": true } }))
            }
            "turn/start" => {
                let thread_id = params
                    .get("threadId")
                    .or_else(|| params.get("thread_id"))
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| "threadId is required".to_string())?
                    .to_string();
                let prompt = extract_user_text_from_turn_input(&params)
                    .ok_or_else(|| "No text input provided for compatible CLI turn".to_string())?;

                let (turn_id, item_id, cli_bin, base_args, interrupt_signal) = {
                    let mut state = self.compatible.lock().await;
                    if !state.threads.contains_key(&thread_id) {
                        return Err("thread not found".to_string());
                    }
                    let turn_id = format!("compat-turn-{}", state.next_turn_seq);
                    state.next_turn_seq += 1;
                    let item_id = format!("compat-item-{}", state.next_item_seq);
                    state.next_item_seq += 1;
                    let interrupt_signal = Arc::new(AtomicBool::new(false));
                    state
                        .active_turn_interrupts
                        .insert(turn_id.clone(), Arc::clone(&interrupt_signal));
                    (
                        turn_id,
                        item_id,
                        state.cli_bin.clone(),
                        state.cli_args.clone(),
                        interrupt_signal,
                    )
                };

                self.emit_app_message(json!({
                    "method": "turn/started",
                    "params": {
                        "threadId": thread_id,
                        "turn": { "id": turn_id, "threadId": thread_id },
                    }
                }));
                self.emit_app_message(json!({
                    "method": "item/started",
                    "params": {
                        "threadId": thread_id,
                        "item": {
                            "id": item_id,
                            "type": "agentMessage",
                            "status": "inProgress",
                        },
                    }
                }));

                let session = Arc::clone(self);
                let thread_id_clone = thread_id.clone();
                let turn_id_clone = turn_id.clone();
                let item_id_clone = item_id.clone();
                tokio::spawn(async move {
                    let (cli_args, use_stdin_prompt) =
                        build_compatible_cli_invocation(&base_args, &prompt);
                    let worker_session = Arc::clone(&session);
                    let result = tokio::task::spawn_blocking(move || {
                        run_compatible_pty_command(
                            worker_session.entry.path.clone(),
                            cli_bin,
                            cli_args,
                            prompt,
                            use_stdin_prompt,
                            Arc::clone(&interrupt_signal),
                        )
                    })
                    .await;

                    let output_result = match result {
                        Ok(inner) => inner,
                        Err(error) => Err(format!("Compatible PTY sidecar worker failed: {error}")),
                    };

                    {
                        let mut state = session.compatible.lock().await;
                        state.active_turn_interrupts.remove(&turn_id_clone);
                    }

                    match output_result {
                        Ok(output) => {
                            for chunk in output.as_bytes().chunks(1024) {
                                let delta = String::from_utf8_lossy(chunk).to_string();
                                if delta.is_empty() {
                                    continue;
                                }
                                session.emit_app_message(json!({
                                    "method": "item/agentMessage/delta",
                                    "params": {
                                        "threadId": thread_id_clone,
                                        "itemId": item_id_clone,
                                        "delta": delta,
                                    }
                                }));
                            }
                            session.emit_app_message(json!({
                                "method": "item/completed",
                                "params": {
                                    "threadId": thread_id_clone,
                                    "item": {
                                        "id": item_id_clone,
                                        "type": "agentMessage",
                                        "status": "completed",
                                        "text": output,
                                    }
                                }
                            }));
                            session.emit_app_message(json!({
                                "method": "turn/completed",
                                "params": {
                                    "threadId": thread_id_clone,
                                    "turn": { "id": turn_id_clone, "threadId": thread_id_clone },
                                }
                            }));
                            let now = now_epoch_seconds();
                            let preview = output
                                .lines()
                                .map(str::trim)
                                .find(|line| !line.is_empty())
                                .map(|line| line.chars().take(120).collect::<String>())
                                .unwrap_or_else(|| "Response completed".to_string());
                            let mut state = session.compatible.lock().await;
                            if let Some(thread) = state.threads.get_mut(&thread_id_clone) {
                                thread.updated_at = now;
                                thread.preview = preview;
                            }
                        }
                        Err(error) => {
                            session.emit_app_message(json!({
                                "method": "error",
                                "params": {
                                    "threadId": thread_id_clone,
                                    "turnId": turn_id_clone,
                                    "error": { "message": error },
                                    "willRetry": false,
                                }
                            }));
                        }
                    }
                });

                Ok(json!({
                    "result": {
                        "turn": {
                            "id": turn_id,
                            "threadId": thread_id,
                        }
                    }
                }))
            }
            "turn/interrupt" => {
                let turn_id = params
                    .get("turnId")
                    .or_else(|| params.get("turn_id"))
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| "turnId is required".to_string())?
                    .to_string();
                let state = self.compatible.lock().await;
                if let Some(signal) = state.active_turn_interrupts.get(&turn_id) {
                    signal.store(true, Ordering::SeqCst);
                }
                Ok(json!({ "result": { "ok": true } }))
            }
            "model/list" => Ok(json!({ "result": { "data": [] } })),
            "skills/list" => Ok(json!({ "result": { "data": [] } })),
            "app/list" => Ok(json!({ "result": { "data": [] } })),
            "mcpServerStatus/list" => Ok(json!({ "result": { "data": [] } })),
            "collaborationMode/list" => Ok(json!({ "result": { "data": [] } })),
            "review/start" => Err("Review is not available in compatible CLI mode.".to_string()),
            "account/read" => Ok(json!({
                "result": {
                    "authMode": "unknown",
                }
            })),
            "account/rateLimits/read" => Ok(json!({ "result": {} })),
            "account/login/start" => {
                Err("Login is not available in compatible CLI mode.".to_string())
            }
            "account/login/cancel" => Ok(json!({ "result": { "cancelled": false } })),
            _ => Err(format!(
                "Method `{method}` is not available in compatible CLI mode."
            )),
        }
    }

    pub(crate) async fn send_request(
        self: &Arc<Self>,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        if self.mode == SessionMode::CompatiblePty {
            return self.send_request_compatible(method, params).await;
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        self.write_message(json!({ "id": id, "method": method, "params": params }))
            .await?;
        rx.await.map_err(|_| "request canceled".to_string())
    }

    pub(crate) async fn send_notification(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), String> {
        if self.mode == SessionMode::CompatiblePty {
            return Ok(());
        }
        let value = if let Some(params) = params {
            json!({ "method": method, "params": params })
        } else {
            json!({ "method": method })
        };
        self.write_message(value).await
    }

    pub(crate) async fn send_response(&self, id: Value, result: Value) -> Result<(), String> {
        if self.mode == SessionMode::CompatiblePty {
            return Ok(());
        }
        self.write_message(json!({ "id": id, "result": result }))
            .await
    }
}

pub(crate) fn build_codex_path_env(codex_bin: Option<&str>) -> Option<String> {
    let mut paths: Vec<PathBuf> = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default();

    let mut extras: Vec<PathBuf> = Vec::new();

    #[cfg(not(target_os = "windows"))]
    {
        extras.extend(
            [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/usr/bin",
                "/bin",
                "/usr/sbin",
                "/sbin",
            ]
            .into_iter()
            .map(PathBuf::from),
        );

        if let Ok(home) = env::var("HOME") {
            let home_path = Path::new(&home);
            extras.push(home_path.join(".local/bin"));
            extras.push(home_path.join(".local/share/mise/shims"));
            extras.push(home_path.join(".cargo/bin"));
            extras.push(home_path.join(".bun/bin"));
            let nvm_root = home_path.join(".nvm/versions/node");
            if let Ok(entries) = std::fs::read_dir(nvm_root) {
                for entry in entries.flatten() {
                    let bin_path = entry.path().join("bin");
                    if bin_path.is_dir() {
                        extras.push(bin_path);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            extras.push(Path::new(&appdata).join("npm"));
        }
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            extras.push(
                Path::new(&local_app_data)
                    .join("Microsoft")
                    .join("WindowsApps"),
            );
        }
        if let Ok(home) = env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
            let home_path = Path::new(&home);
            extras.push(home_path.join(".cargo").join("bin"));
            extras.push(home_path.join("scoop").join("shims"));
        }
        if let Ok(program_data) = env::var("PROGRAMDATA") {
            extras.push(Path::new(&program_data).join("chocolatey").join("bin"));
        }
    }

    if let Some(bin_path) = codex_bin.filter(|value| !value.trim().is_empty()) {
        if let Some(parent) = Path::new(bin_path).parent() {
            extras.push(parent.to_path_buf());
        }
    }

    for extra in extras {
        if !paths.iter().any(|path| path == &extra) {
            paths.push(extra);
        }
    }

    if paths.is_empty() {
        return None;
    }

    env::join_paths(paths)
        .ok()
        .map(|joined| joined.to_string_lossy().to_string())
}

pub(crate) fn build_codex_command_with_bin(
    codex_bin: Option<String>,
    codex_args: Option<&str>,
    args: Vec<String>,
) -> Result<Command, String> {
    let bin = codex_bin
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "codex".into());

    let path_env = build_codex_path_env(codex_bin.as_deref());
    let mut command_args = parse_codex_args(codex_args)?;
    command_args.extend(args);

    #[cfg(target_os = "windows")]
    let mut command = {
        let bin_trimmed = bin.trim();
        let resolved = resolve_windows_executable(bin_trimmed, path_env.as_deref());
        let resolved_path = resolved
            .as_deref()
            .unwrap_or_else(|| Path::new(bin_trimmed));
        let ext = resolved_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());

        if matches!(ext.as_deref(), Some("cmd") | Some("bat")) {
            let mut command = tokio_command("cmd");
            let command_line = build_cmd_c_command(resolved_path, &command_args)?;
            command.arg("/D");
            command.arg("/S");
            command.arg("/C");
            command.arg(command_line);
            command
        } else {
            let mut command = tokio_command(resolved_path);
            command.args(command_args);
            command
        }
    };

    #[cfg(not(target_os = "windows"))]
    let mut command = {
        let mut command = tokio_command(bin.trim());
        command.args(command_args);
        command
    };

    if let Some(path_env) = path_env {
        command.env("PATH", path_env);
    }
    Ok(command)
}

pub(crate) async fn check_codex_installation(
    codex_bin: Option<String>,
) -> Result<Option<String>, String> {
    let mut command = build_codex_command_with_bin(codex_bin, None, vec!["--version".to_string()])?;
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let output = match timeout(Duration::from_secs(5), command.output()).await {
        Ok(result) => result.map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                "Codex CLI not found. Install Codex and ensure `codex` is on your PATH.".to_string()
            } else {
                e.to_string()
            }
        })?,
        Err(_) => {
            return Err(
                "Timed out while checking Codex CLI. Make sure `codex --version` runs in Terminal."
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
                "Codex CLI failed to start. Try running `codex --version` in Terminal.".to_string(),
            );
        }
        return Err(format!(
            "Codex CLI failed to start: {detail}. Try running `codex --version` in Terminal."
        ));
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if version.is_empty() {
        None
    } else {
        Some(version)
    })
}

async fn check_cli_invocation_available(
    cli_bin: Option<String>,
    cli_args: Option<&str>,
) -> Result<(), String> {
    let mut command = build_codex_command_with_bin(cli_bin, cli_args, vec!["--help".to_string()])?;
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());
    match timeout(Duration::from_secs(5), command.output()).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => {
            if error.kind() == ErrorKind::NotFound {
                Err("Selected CLI was not found. Check the configured binary path.".to_string())
            } else {
                Err(format!("Failed to start selected CLI: {error}"))
            }
        }
        Err(_) => Err("Timed out while checking selected CLI availability.".to_string()),
    }
}

pub(crate) async fn spawn_workspace_session<E: EventSink>(
    entry: WorkspaceEntry,
    default_codex_bin: Option<String>,
    codex_args: Option<String>,
    codex_home: Option<PathBuf>,
    client_version: String,
    event_sink: E,
) -> Result<Arc<WorkspaceSession>, String> {
    let codex_bin = default_codex_bin
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            entry
                .codex_bin
                .clone()
                .filter(|value| !value.trim().is_empty())
        });
    let cli_bin = codex_bin
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "codex".to_string());
    let parsed_cli_args = parse_codex_args(codex_args.as_deref())?;
    let rpc_capable = {
        let mut probe = build_codex_command_with_bin(
            codex_bin.clone(),
            codex_args.as_deref(),
            vec!["app-server".to_string(), "--help".to_string()],
        )?;
        probe.current_dir(&entry.path);
        if let Some(codex_home) = codex_home.clone() {
            probe.env("CODEX_HOME", codex_home);
        }
        probe.stdout(std::process::Stdio::null());
        probe.stderr(std::process::Stdio::null());
        match timeout(Duration::from_secs(5), probe.output()).await {
            Ok(Ok(output)) => output.status.success(),
            _ => false,
        }
    };

    let workspace_id_for_emitter = entry.id.clone();
    let sink_for_emitter = event_sink.clone();
    let app_event_emitter: Arc<dyn Fn(Value) + Send + Sync> = Arc::new(move |message: Value| {
        sink_for_emitter.emit_app_server_event(AppServerEvent {
            workspace_id: workspace_id_for_emitter.clone(),
            message,
        });
    });

    if !rpc_capable {
        check_cli_invocation_available(codex_bin.clone(), codex_args.as_deref()).await?;
        let session = Arc::new(WorkspaceSession {
            entry: entry.clone(),
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            mode: SessionMode::CompatiblePty,
            compatible: Mutex::new(CompatibleSessionState::new(cli_bin, parsed_cli_args)),
            app_event_emitter: Arc::clone(&app_event_emitter),
            background_thread_callbacks: Mutex::new(HashMap::new()),
        });
        session.emit_app_message(json!({
            "method": "codex/connected",
            "params": {
                "workspaceId": entry.id.clone(),
                "mode": "compatible",
            }
        }));
        return Ok(session);
    }

    let mut command = build_codex_command_with_bin(
        codex_bin,
        codex_args.as_deref(),
        vec!["app-server".to_string()],
    )?;
    command.current_dir(&entry.path);
    if let Some(codex_home) = codex_home {
        command.env("CODEX_HOME", codex_home);
    }
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdin = child.stdin.take().ok_or("missing stdin")?;
    let stdout = child.stdout.take().ok_or("missing stdout")?;
    let stderr = child.stderr.take().ok_or("missing stderr")?;

    let session = Arc::new(WorkspaceSession {
        entry: entry.clone(),
        child: Mutex::new(Some(child)),
        stdin: Mutex::new(Some(stdin)),
        pending: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(1),
        mode: SessionMode::JsonRpc,
        compatible: Mutex::new(CompatibleSessionState::new(String::new(), Vec::new())),
        app_event_emitter: Arc::clone(&app_event_emitter),
        background_thread_callbacks: Mutex::new(HashMap::new()),
    });

    let session_clone = Arc::clone(&session);
    let workspace_id = entry.id.clone();
    let event_sink_clone = event_sink.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(err) => {
                    let payload = AppServerEvent {
                        workspace_id: workspace_id.clone(),
                        message: json!({
                            "method": "codex/parseError",
                            "params": { "error": err.to_string(), "raw": line },
                        }),
                    };
                    event_sink_clone.emit_app_server_event(payload);
                    continue;
                }
            };

            let maybe_id = value.get("id").and_then(|id| id.as_u64());
            let has_method = value.get("method").is_some();
            let has_result_or_error = value.get("result").is_some() || value.get("error").is_some();

            // Check if this event is for a background thread
            let thread_id = extract_thread_id(&value);

            if let Some(id) = maybe_id {
                if has_result_or_error {
                    if let Some(tx) = session_clone.pending.lock().await.remove(&id) {
                        let _ = tx.send(value);
                    }
                } else if has_method {
                    // Check for background thread callback
                    let mut sent_to_background = false;
                    if let Some(ref tid) = thread_id {
                        let callbacks = session_clone.background_thread_callbacks.lock().await;
                        if let Some(tx) = callbacks.get(tid) {
                            let _ = tx.send(value.clone());
                            sent_to_background = true;
                        }
                    }
                    // Don't emit to frontend if this is a background thread event
                    if !sent_to_background {
                        let payload = AppServerEvent {
                            workspace_id: workspace_id.clone(),
                            message: value,
                        };
                        event_sink_clone.emit_app_server_event(payload);
                    }
                } else if let Some(tx) = session_clone.pending.lock().await.remove(&id) {
                    let _ = tx.send(value);
                }
            } else if has_method {
                // Check for background thread callback
                let mut sent_to_background = false;
                if let Some(ref tid) = thread_id {
                    let callbacks = session_clone.background_thread_callbacks.lock().await;
                    if let Some(tx) = callbacks.get(tid) {
                        let _ = tx.send(value.clone());
                        sent_to_background = true;
                    }
                }
                // Don't emit to frontend if this is a background thread event
                if !sent_to_background {
                    let payload = AppServerEvent {
                        workspace_id: workspace_id.clone(),
                        message: value,
                    };
                    event_sink_clone.emit_app_server_event(payload);
                }
            }
        }
    });

    let workspace_id = entry.id.clone();
    let event_sink_clone = event_sink.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let payload = AppServerEvent {
                workspace_id: workspace_id.clone(),
                message: json!({
                    "method": "codex/stderr",
                    "params": { "message": line },
                }),
            };
            event_sink_clone.emit_app_server_event(payload);
        }
    });

    let init_params = build_initialize_params(&client_version);
    let init_result = timeout(
        Duration::from_secs(15),
        session.send_request("initialize", init_params),
    )
    .await;
    let init_response = match init_result {
        Ok(response) => response,
        Err(_) => {
            session.terminate_process().await;
            return Err(
                "Codex app-server did not respond to initialize. Check that `codex app-server` works in Terminal."
                    .to_string(),
            );
        }
    };
    init_response?;
    session.send_notification("initialized", None).await?;

    let payload = AppServerEvent {
        workspace_id: entry.id.clone(),
        message: json!({
            "method": "codex/connected",
            "params": { "workspaceId": entry.id.clone() }
        }),
    };
    event_sink.emit_app_server_event(payload);

    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::{
        build_compatible_cli_invocation, build_initialize_params, extract_thread_id,
        extract_user_text_from_turn_input,
    };
    use serde_json::json;

    #[test]
    fn extract_thread_id_reads_camel_case() {
        let value = json!({ "params": { "threadId": "thread-123" } });
        assert_eq!(extract_thread_id(&value), Some("thread-123".to_string()));
    }

    #[test]
    fn extract_thread_id_reads_snake_case() {
        let value = json!({ "params": { "thread_id": "thread-456" } });
        assert_eq!(extract_thread_id(&value), Some("thread-456".to_string()));
    }

    #[test]
    fn extract_thread_id_returns_none_when_missing() {
        let value = json!({ "params": {} });
        assert_eq!(extract_thread_id(&value), None);
    }

    #[test]
    fn build_initialize_params_enables_experimental_api() {
        let params = build_initialize_params("1.2.3");
        assert_eq!(
            params
                .get("capabilities")
                .and_then(|caps| caps.get("experimentalApi"))
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn compatible_cli_invocation_replaces_prompt_template() {
        let (args, use_stdin_prompt) =
            build_compatible_cli_invocation(&["-p".to_string(), "{prompt}".to_string()], "hello");
        assert_eq!(args, vec!["-p".to_string(), "hello".to_string()]);
        assert!(!use_stdin_prompt);
    }

    #[test]
    fn compatible_cli_invocation_falls_back_to_stdin() {
        let (args, use_stdin_prompt) =
            build_compatible_cli_invocation(&["--format".to_string(), "text".to_string()], "hello");
        assert_eq!(args, vec!["--format".to_string(), "text".to_string()]);
        assert!(use_stdin_prompt);
    }

    #[test]
    fn extracts_user_text_from_turn_input_items() {
        let params = json!({
            "input": [
                { "type": "text", "text": " first " },
                { "type": "localImage", "path": "/tmp/example.png" },
                { "type": "text", "text": "second" }
            ]
        });
        assert_eq!(
            extract_user_text_from_turn_input(&params),
            Some("first\n\nsecond".to_string())
        );
    }
}
