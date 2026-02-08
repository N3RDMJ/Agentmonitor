use serde::Serialize;

use crate::backend::app_server::check_cli_installation;

#[derive(Debug, Serialize, Clone)]
pub(crate) struct DetectedClis {
    pub(crate) codex: Option<String>,
    pub(crate) claude: Option<String>,
    pub(crate) gemini: Option<String>,
    pub(crate) cursor: Option<String>,
}

/// Probes default bin names on PATH; ignores user-configured custom bin overrides.
pub(crate) async fn detect_installed_clis() -> DetectedClis {
    let (codex, claude, gemini, cursor) = tokio::join!(
        probe_cli(Some("codex".to_string()), "Codex"),
        probe_cli(Some("claude".to_string()), "Claude"),
        probe_cli(Some("gemini".to_string()), "Gemini"),
        probe_cli(Some("cursor".to_string()), "Cursor"),
    );
    DetectedClis {
        codex,
        claude,
        gemini,
        cursor,
    }
}

async fn probe_cli(bin: Option<String>, name: &str) -> Option<String> {
    check_cli_installation(bin, name).await.ok().flatten()
}
