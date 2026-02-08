# AgentMonitor

![AgentMonitor home](docs/images/app-home.png)
![AgentMonitor CLI Backend settings](docs/images/settings-cli-backend.png)

Agent Monitor is a Tauri desktop app for orchestrating multiple coding CLIs across local workspaces. It combines workspace/thread orchestration, approvals, reviews, git tooling, prompt workflows, and terminal utilities in one UI.

## Quickstart (npm)

```bash
npm install
npm run tauri dev
```

## Features

### CLI Compatibility Tiers

- **Full mode (Codex):** bidirectional JSON-RPC app-server integration with full interactive capabilities.
- **Compatible mode (Gemini CLI, Cursor CLI, Claude Code):** core orchestration support with capability-gated UI; advanced controls that require full duplex JSON-RPC are disabled when unsupported.
- Settings surfaces the active mode and explains what is available for the selected CLI.

### Workspaces & Threads

- Add and persist workspaces, group/sort them, and open recent thread activity from the home dashboard.
- Thread management: pin, rename, archive, fork, resume, copy.
- Per-thread drafts, activity timestamps, and conversation persistence.

### Composer & Agent Controls

- Composer queue with image attachments (picker, drag/drop, paste).
- Autocomplete for skills (`$`), prompts, reviews (`/review`), and file paths (`@`).
- Model picker, collaboration modes (when supported), reasoning effort, and access mode controls.
- Dictation (Whisper) with hold-to-talk shortcuts and live waveform.
- Tool/reasoning/diff item rendering and approval request handling.

### Git & GitHub

- Diff stats and staged/unstaged file diffs with stage/revert controls.
- Commit flow and branch operations (list/checkout/create).
- GitHub Issues/PR integrations through `gh` (when installed).

### Files, Prompts, and UI

- File tree with search and reveal/open-in-system helpers.
- Prompt library for global/workspace prompts (create/edit/delete/move/run).
- Resizable panels, responsive layouts (desktop/tablet/phone), update toasts, notification controls, and platform-specific window effects.

## Requirements

### For Users (Pre-built Release)

Install at least one supported CLI and ensure it is in `PATH` (or configure the binary path in Settings):

- Codex CLI
- Gemini CLI
- Cursor CLI
- Claude Code CLI

Optional tools:

- Git CLI (`git`) for repository features
- GitHub CLI (`gh`) for Issues/PR integration

### For Developers (Building from Source)

- Node.js + npm
- Rust toolchain (stable)
- CMake (required by native dependencies)
- LLVM/Clang (required on Windows for bindgen-backed dependencies)
- A supported CLI available in `PATH`

If you hit native build errors, run:

```bash
npm run doctor
```

## Local Development

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

Artifacts are generated under `src-tauri/target/release/bundle/` (platform-specific subfolders).

### Automated Builds (GitHub Actions)

The repo includes release/build workflows (including DMG/release automation).

See `docs/RELEASE.md` for signing/notarization and release setup.

### Windows (opt-in)

Windows builds use a separate config path to avoid macOS-only window effects:

```bash
npm run tauri:build:win
```

Expected bundle output:

- `src-tauri/target/release/bundle/nsis/`
- `src-tauri/target/release/bundle/msi/`

## Validation

Run before opening/merging PRs:

```bash
npm run lint
npm run test
npm run typecheck
```

If Rust backend code changed:

```bash
cd src-tauri
cargo check
```

## Project Structure

```text
src/
  App.tsx                 composition root
  features/               feature slices (hooks/components)
  services/               Tauri IPC + event wrappers
  styles/                 CSS by UI area
  types.ts                shared UI types
src-tauri/src/
  lib.rs                  Tauri command registry
  backend/                app-server/session infrastructure
  shared/                 shared core logic (app + daemon)
  bin/codex_monitor_daemon.rs
                          daemon transport/wiring
```

## Notes

- Workspaces persist in app-data `workspaces.json`.
- App settings persist in app-data `settings.json`.
- UI preferences (panel state and other client-side UX state) persist in `localStorage`.
- Supported Codex config keys can be synced to `$CODEX_HOME/config.toml`.
- Compatible-mode approval handling sends explicit server responses for hidden/unsupported approvals to prevent hanging requests.

## Tauri IPC Surface

Frontend wrappers live in `src/services/tauri.ts` and map to commands in `src-tauri/src/lib.rs`.

Core command groups include:

- Workspaces: `list_workspaces`, `add_workspace`, `add_worktree`, `remove_workspace`, `connect_workspace`, `update_workspace_settings`
- Threads/runtime: `start_thread`, `list_threads`, `resume_thread`, `archive_thread`, `send_user_message`, `turn_interrupt`, `respond_to_server_request`
- Agent capabilities: `start_review`, `model_list`, `account_rate_limits`, `skills_list`, `apps_list`
- Git/files: `get_git_status`, `get_git_diffs`, `get_git_log`, `list_git_branches`, `checkout_git_branch`, `list_workspace_files`
