# Agent Monitor

A multi-agent desktop app for orchestrating AI coding agents across local workspaces. Built with Tauri (React + Vite frontend, Rust backend), Agent Monitor lets you manage conversations, workspaces, and git operations through a unified interface — regardless of which CLI you use.

## Features

### Multi-Agent Support

- **Supported CLIs**: Codex CLI, Gemini CLI, Cursor CLI, and Claude Code.
- Per-CLI settings: configure binary path, arguments, and home directory in Settings.
- Custom adapter pattern for CLIs that don't implement the app-server protocol (e.g., Claude Code uses a headless CLI adapter with stderr event routing).
- Switch active CLI in **Settings → CLI Backend**.

### Workspaces & Threads

- Add and persist workspaces, group/sort them, and jump into recent agent activity from the home dashboard.
- Each workspace gets its own agent session with conversation history.
- Thread management: pin/rename/archive/copy, per-thread drafts, and stop/interrupt in-flight turns.

### Composer & Agent Controls

- Compose with queueing plus image attachments (picker, drag/drop, paste).
- Autocomplete for skills (`$`), prompts (`/prompts:`), reviews (`/review`), and file paths (`@`).
- Model picker, collaboration modes (when enabled), reasoning effort, access mode, and context usage ring.
- Syntax-highlighted code blocks in agent responses.
- Dictation with hold-to-talk shortcuts and live waveform (Whisper).
- Render reasoning/tool/diff items and handle approval prompts.

### Git & GitHub

- Diff stats, staged/unstaged file diffs, revert/stage controls, and commit log.
- Branch list with checkout/create plus upstream ahead/behind counts.
- GitHub Issues and Pull Requests via `gh` (lists, diffs, comments) and open commits/PRs in the browser.
- PR composer: "Ask PR" to send PR context into a new agent thread.

### Files & Prompts

- File tree with search, file-type icons, and Reveal in Finder/Explorer.
- Prompt library for global/workspace prompts: create/edit/delete/move and run in current or new threads.
- Agent profiles for per-workspace agent configuration.

### UI & Experience

- Resizable sidebar/right/plan/terminal/debug panels with persisted sizes.
- Responsive layouts (desktop/tablet/phone) with tabbed navigation.
- CLI-filtered usage dashboard: sidebar usage meter and credits display for account rate limits, plus a home usage snapshot.
- Sandbox bootstrap toggle for agent session isolation.
- Terminal dock with multiple tabs for background commands (experimental).
- In-app updates with toast-driven download/install, debug panel copy/clear, sound notifications, plus platform-specific window effects (macOS overlay title bar + vibrancy) and a reduced transparency toggle.

## Requirements

### For Users (Pre-built Release)

1. **Install at least one supported CLI**:
   - [Codex CLI](https://github.com/openai/codex) — `npm install -g @openai/codex`
   - [Gemini CLI](https://github.com/google-gemini/gemini-cli) — `npm install -g @google/gemini-cli`
   - [Cursor CLI](https://docs.cursor.com/cli) — ships with Cursor
   - [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — `npm install -g @anthropic-ai/claude-code`

2. **Select active CLI** in Settings → CLI Backend

3. **Download Agent Monitor** from the releases page and open it

The app will detect the active CLI automatically. If not found, configure the path in Settings → Run Doctor.

### Homebrew (macOS)

You can install Agent Monitor with Homebrew:

```bash
brew tap N3RDMJ/tap
brew install --cask agent-monitor
```

Upgrade and uninstall:

```bash
brew upgrade --cask agent-monitor
brew uninstall --cask agent-monitor
```

**Optional:**
- Git CLI (for Git panel features)
- GitHub CLI (`gh`) for the Issues panel

### For Developers (Building from Source)

- Node.js + npm
- Rust toolchain (stable)
- CMake (required for native dependencies; dictation/Whisper uses it)
- LLVM/Clang (required on Windows to build dictation dependencies via bindgen)
- Agent CLI installed on your system and available in `PATH`
- Git CLI (used for worktree operations)
- GitHub CLI (`gh`) for the Issues panel (optional)

If you hit native build errors, run:

```bash
npm run doctor
```

## Getting Started

Install dependencies:

```bash
npm install
```

Run in dev mode:

```bash
npm run tauri dev
```

## Release Build

Build the production Tauri bundle:

```bash
npm run tauri build
```

Artifacts will be in `src-tauri/target/release/bundle/` (platform-specific subfolders).

### Automated Builds (GitHub Actions)

The repository includes workflows for automated builds:

- **Build DMG**: Builds unsigned or signed DMG files on demand
- **Release**: Full release workflow with code signing and notarization

See [docs/RELEASE.md](docs/RELEASE.md) for setting up code signing and notarization.

### Windows (opt-in)

Windows builds are opt-in and use a separate Tauri config file to avoid macOS-only window effects.

```bash
npm run tauri:build:win
```

Artifacts will be in:

- `src-tauri/target/release/bundle/nsis/` (installer exe)
- `src-tauri/target/release/bundle/msi/` (msi)

Note: building from source on Windows requires LLVM/Clang (for `bindgen` / `libclang`) in addition to CMake.

## Type Checking

Run the TypeScript checker (no emit):

```bash
npm run typecheck
```

Note: `npm run build` also runs `tsc` before bundling the frontend.

## Project Structure

```
src/
  features/         feature-sliced UI (24 slices)
  services/         Tauri IPC, event hub, toasts
  styles/           CSS by area
  utils/            pure helpers
  types.ts          shared UI types
src-tauri/
  src/
    backend/        agent sessions + CLI adapters
    shared/         domain cores (codex, git, workspaces, settings, files, ...)
    codex/          Codex CLI wiring
    gemini/         Gemini CLI wiring
    files/          file I/O wiring
    workspaces/     workspace wiring
    settings/       settings wiring
    lib.rs          Tauri command registry
    bin/            daemon binaries
```

## Notes

- Workspaces persist to `workspaces.json` under the app data directory.
- App settings persist to `settings.json` under the app data directory (CLI path, default access mode, UI scale).
- Feature settings are supported in the UI and synced to `$CODEX_HOME/config.toml` (or `~/.codex/config.toml`) on load/save. Stable: Collaboration modes (`features.collaboration_modes`), personality (`personality`), Steer mode (`features.steer`), and Background terminal (`features.unified_exec`). Experimental: Collab mode (`features.collab`) and Apps (`features.apps`).
- On launch and on window focus, the app reconnects and refreshes thread lists for each workspace.
- The backend spawns the active CLI for each conversation turn; see `src-tauri/src/backend/` for session and adapter implementations.
- UI state (panel sizes, reduced transparency toggle, recent thread activity) is stored in `localStorage`.

## Tauri IPC Surface

Frontend calls live in `src/services/tauri.ts` and map to commands in `src-tauri/src/lib.rs`. Core commands include:

- Workspace lifecycle: `list_workspaces`, `add_workspace`, `add_worktree`, `remove_workspace`, `remove_worktree`, `connect_workspace`, `update_workspace_settings`.
- Threads: `start_thread`, `list_threads`, `resume_thread`, `archive_thread`, `send_user_message`, `turn_interrupt`, `respond_to_server_request`.
- Reviews + models: `start_review`, `model_list`, `account_rate_limits`, `skills_list`.
- Git + files: `get_git_status`, `get_git_diffs`, `get_git_log`, `get_git_remote`, `list_git_branches`, `checkout_git_branch`, `create_git_branch`, `list_workspace_files`.

## Further Reading

- [docs/RELEASE.md](docs/RELEASE.md) — Release workflow and code signing
- [docs/INSTALLATION.md](docs/INSTALLATION.md) — Platform-specific installation notes
