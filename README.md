# AgentMonitor

![AgentMonitor home](docs/images/app-home.png)
![AgentMonitor CLI Backend settings](docs/images/settings-cli-backend.png)

AgentMonitor is a desktop Tauri app for running coding agents across local workspaces with one UI for threads, approvals, reviews, git workflows, prompts, and terminal tooling.

## Quickstart (npm)

```bash
npm install
npm run tauri dev
```

## What It Supports

### CLI Backends

- `Codex` (full mode): bidirectional JSON-RPC app-server integration.
- `Gemini CLI`, `Cursor CLI`, `Claude Code` (compatible mode): capability-gated UI with PTY sidecar fallback for turn execution when full app-server semantics are unavailable.

### Capability Tiers

- Full mode enables the complete interactive surface (approvals, interrupt, MCP/apps parity, collaboration features where available).
- Compatible mode keeps core workflows working and disables unsupported controls in Settings and runtime surfaces.

### Workspace and Thread Operations

- Add, connect, group, clone, and worktree-manage repositories.
- Persist thread lists and thread activity per workspace.
- Pin, rename, archive, fork, and resume threads.

### Composer and Runtime

- Prompt composer with queueing and image attachments.
- Slash and symbol workflows (`/review`, prompts, skills, file autocomplete).
- Model/reasoning/access controls.
- Runtime event stream for messages, tools, diffs, and reasoning items.

### Git and Repo Flow

- Status, staged/unstaged diffs, file operations, commit/push/pull/fetch/sync.
- Branch switch/create workflows and remote branch helpers.
- PR/issue helpers via `gh` when configured.

## Requirements

### End Users (Prebuilt App)

Install at least one supported CLI and make it available in `PATH` (or configure its binary path in Settings):

- Codex CLI
- Gemini CLI
- Cursor CLI
- Claude Code CLI

Optional tools:

- `git` for workspace/repo features
- `gh` for GitHub issue/PR features

### Developers

- Node.js + npm
- Rust stable toolchain
- CMake
- LLVM/Clang (required on Windows for bindgen-backed dependencies)

## Local Development

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

Windows opt-in build:

```bash
npm run tauri:build:win
```

## Validation

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

## Project Layout

```text
src/
  App.tsx                 app composition root
  features/               feature slices (hooks/components)
  services/               Tauri IPC/event wrappers
  styles/                 UI styling
  types.ts                shared UI types
src-tauri/src/
  lib.rs                  Tauri command registry
  backend/                app-server/session infrastructure
  shared/                 core logic shared by app/daemon
  bin/codex_monitor_daemon.rs
                          daemon transport/wiring
```

## Persistence

- Workspaces: app data `workspaces.json`
- App settings: app data `settings.json`
- UI preferences and panel state: `localStorage`

## Notes

- Settings can sync supported Codex config keys to `$CODEX_HOME/config.toml`.
- Compatible-mode approvals are never left hanging: hidden/unsupported approval requests receive explicit responses.
- This repo also supports daemon/remote mode for backend command transport.

## Release and Signing

See `docs/RELEASE.md` for release workflow details (bundles, signing, notarization, and CI release steps).
