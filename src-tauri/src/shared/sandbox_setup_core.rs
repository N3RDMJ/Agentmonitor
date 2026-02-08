use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

const GONDOLIN_MCP_SERVER: &str = "gondolin";

fn gondolin_command_spec() -> (String, Vec<String>) {
    (
        "npx".to_string(),
        vec![
            "-y".to_string(),
            "@earendil-works/gondolin".to_string(),
            "mcp".to_string(),
        ],
    )
}

fn command_in_workspace(workspace_path: &Path, program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .current_dir(workspace_path)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn ensure_codex_mcp_server(workspace_path: &Path) {
    if command_in_workspace(
        workspace_path,
        "codex",
        &["mcp", "get", GONDOLIN_MCP_SERVER],
    ) {
        return;
    }
    let (command, args) = gondolin_command_spec();
    let mut cli_args: Vec<&str> = vec!["mcp", "add", GONDOLIN_MCP_SERVER, "--"];
    cli_args.push(command.as_str());
    cli_args.extend(args.iter().map(|value| value.as_str()));
    let _ = command_in_workspace(workspace_path, "codex", &cli_args);
}

fn ensure_claude_mcp_server(workspace_path: &Path) {
    if command_in_workspace(
        workspace_path,
        "claude",
        &["mcp", "get", GONDOLIN_MCP_SERVER],
    ) {
        return;
    }
    let (command, args) = gondolin_command_spec();
    let mut cli_args: Vec<&str> = vec![
        "mcp",
        "add",
        "--scope",
        "project",
        GONDOLIN_MCP_SERVER,
        "--",
    ];
    cli_args.push(command.as_str());
    cli_args.extend(args.iter().map(|value| value.as_str()));
    let _ = command_in_workspace(workspace_path, "claude", &cli_args);
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value
        .as_object_mut()
        .expect("value was initialized to an object")
}

fn upsert_gemini_mcp_config(root: &mut Value) {
    let (command, args) = gondolin_command_spec();
    let server_payload = json!({
        "command": command,
        "args": args,
    });

    let root_object = ensure_object(root);

    let mcp_servers = root_object
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    let mcp_servers_object = ensure_object(mcp_servers);
    mcp_servers_object.insert(GONDOLIN_MCP_SERVER.to_string(), server_payload.clone());

    // Gemini configs vary across versions (`mcp.servers` vs `mcpServers`), so write both.
    let mcp = root_object
        .entry("mcp".to_string())
        .or_insert_with(|| json!({}));
    let mcp_object = ensure_object(mcp);
    let servers = mcp_object
        .entry("servers".to_string())
        .or_insert_with(|| json!({}));
    let servers_object = ensure_object(servers);
    servers_object.insert(GONDOLIN_MCP_SERVER.to_string(), server_payload);
}

fn ensure_gemini_mcp_server(gemini_home: Option<PathBuf>) -> Result<(), String> {
    let home = gemini_home
        .or_else(resolve_default_gemini_home_fallback)
        .ok_or_else(|| "Unable to resolve GEMINI_HOME for sandbox setup".to_string())?;
    let settings_path = home.join("settings.json");
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create {}: {err}", parent.display()))?;
    }

    let mut value = if settings_path.exists() {
        let contents = std::fs::read_to_string(&settings_path)
            .map_err(|err| format!("Failed to read {}: {err}", settings_path.display()))?;
        if contents.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str::<Value>(&contents)
                .map_err(|err| format!("Failed to parse {}: {err}", settings_path.display()))?
        }
    } else {
        json!({})
    };

    upsert_gemini_mcp_config(&mut value);
    let serialized = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("Failed to serialize Gemini settings: {err}"))?;
    std::fs::write(&settings_path, format!("{serialized}\n"))
        .map_err(|err| format!("Failed to write {}: {err}", settings_path.display()))
}

fn resolve_default_gemini_home_fallback() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("GEMINI_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    if let Ok(value) = std::env::var("HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join(".gemini"));
        }
    }
    if let Ok(value) = std::env::var("USERPROFILE") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join(".gemini"));
        }
    }
    None
}

pub(crate) fn ensure_workspace_sandbox_setup(
    cli_type: &str,
    workspace_path: &Path,
    cli_home: Option<PathBuf>,
) -> Result<(), String> {
    match cli_type {
        "claude" => {
            ensure_claude_mcp_server(workspace_path);
            Ok(())
        }
        "gemini" => ensure_gemini_mcp_server(cli_home),
        "codex" => {
            // Keep Codex native sandboxing and also ensure Gondolin MCP is available.
            ensure_codex_mcp_server(workspace_path);
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{ensure_workspace_sandbox_setup, upsert_gemini_mcp_config};
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    #[test]
    fn upsert_gemini_mcp_config_adds_both_supported_shapes() {
        let mut value = json!({});
        upsert_gemini_mcp_config(&mut value);

        let server = value
            .get("mcpServers")
            .and_then(|mcp| mcp.get("gondolin"))
            .expect("mcpServers.gondolin should exist");
        assert_eq!(
            server.get("command").and_then(|item| item.as_str()),
            Some("npx")
        );

        let nested = value
            .get("mcp")
            .and_then(|mcp| mcp.get("servers"))
            .and_then(|servers| servers.get("gondolin"))
            .expect("mcp.servers.gondolin should exist");
        assert_eq!(
            nested.get("command").and_then(|item| item.as_str()),
            Some("npx")
        );
    }

    #[test]
    fn upsert_gemini_mcp_config_preserves_existing_keys() {
        let mut value = json!({
            "model": "gemini-2.5-pro",
            "mcpServers": {
                "existing": {
                    "command": "node",
                    "args": ["example.js"]
                }
            }
        });
        upsert_gemini_mcp_config(&mut value);

        assert_eq!(
            value.get("model").and_then(|item| item.as_str()),
            Some("gemini-2.5-pro")
        );
        assert!(value
            .get("mcpServers")
            .and_then(|mcp| mcp.get("existing"))
            .is_some());
        assert!(value
            .get("mcpServers")
            .and_then(|mcp| mcp.get("gondolin"))
            .is_some());
    }

    #[test]
    fn ensure_workspace_sandbox_setup_writes_gemini_settings_file() {
        let workspace_dir = temp_dir("sandbox-workspace");
        let gemini_home = temp_dir("sandbox-gemini-home");

        ensure_workspace_sandbox_setup("gemini", &workspace_dir, Some(gemini_home.clone()))
            .expect("gemini sandbox setup should succeed");

        let settings_path = gemini_home.join("settings.json");
        let contents = fs::read_to_string(&settings_path).expect("settings.json should exist");
        let parsed: serde_json::Value =
            serde_json::from_str(&contents).expect("settings.json should be valid json");
        assert_eq!(
            parsed
                .get("mcpServers")
                .and_then(|mcp| mcp.get("gondolin"))
                .and_then(|server| server.get("command"))
                .and_then(|value| value.as_str()),
            Some("npx")
        );

        let _ = fs::remove_dir_all(workspace_dir);
        let _ = fs::remove_dir_all(gemini_home);
    }
}
