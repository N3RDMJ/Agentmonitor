use std::path::{Path, PathBuf};

use crate::files::io::read_text_file_within;

/// Claude Code stores its settings in `~/.claude/settings.json` (JSON format).
const CLAUDE_SETTINGS_FILENAME: &str = "settings.json";

/// Returns the path to the Claude config directory (e.g. ~/.claude).
pub(crate) fn config_dir_path() -> Option<PathBuf> {
    resolve_default_claude_home()
}

/// Reads the model from the Claude settings.json, if any.
pub(crate) fn read_config_model(claude_home: Option<PathBuf>) -> Result<Option<String>, String> {
    let root = claude_home.or_else(resolve_default_claude_home);
    let Some(root) = root else {
        return Err("Unable to resolve Claude config dir".to_string());
    };
    read_config_model_from_root(&root)
}

fn resolve_default_claude_home() -> Option<PathBuf> {
    crate::claude::home::resolve_default_claude_home()
}

fn read_settings_contents_from_root(root: &Path) -> Result<Option<String>, String> {
    let response = read_text_file_within(
        root,
        CLAUDE_SETTINGS_FILENAME,
        true,                      // root_may_be_missing
        "CLAUDE_CONFIG_DIR",       // root_context
        CLAUDE_SETTINGS_FILENAME,  // filename context
        false,                     // allow_external_symlink_target
    )?;
    if response.exists {
        Ok(Some(response.content))
    } else {
        Ok(None)
    }
}

fn read_config_model_from_root(root: &Path) -> Result<Option<String>, String> {
    let contents = read_settings_contents_from_root(root)?;
    Ok(contents.as_deref().and_then(parse_model_from_json))
}

fn parse_model_from_json(contents: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(contents).ok()?;
    let model = parsed.get("model")?.as_str()?;
    let trimmed = model.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_model_from_json;

    #[test]
    fn parses_model_from_json_settings() {
        let json = r#"{"model": "claude-sonnet-4-5-20250929"}"#;
        assert_eq!(
            parse_model_from_json(json),
            Some("claude-sonnet-4-5-20250929".to_string())
        );
    }

    #[test]
    fn returns_none_for_empty_model() {
        assert_eq!(parse_model_from_json(r#"{"model": ""}"#), None);
        assert_eq!(parse_model_from_json(r#"{"model": "  "}"#), None);
    }

    #[test]
    fn returns_none_for_missing_model() {
        assert_eq!(parse_model_from_json(r#"{}"#), None);
    }

    #[test]
    fn returns_none_for_invalid_json() {
        assert_eq!(parse_model_from_json("not json"), None);
    }
}
