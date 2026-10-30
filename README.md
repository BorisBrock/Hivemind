# Workzone

A desktop board for running AI coding agents across several projects. Write a ticket, move it to
**Ready**, and Workzone runs [Claude Code](https://claude.com/claude-code) or
[OpenCode](https://opencode.ai) headless in the project folder. You follow the agent's work in the
ticket and answer its questions there, instead of juggling a dozen terminal sessions.

- One agent per project at a time, any number of projects in parallel.
- Tickets move through **Planning → Ready → In Progress → Needs Input → Done** on a Kanban board.
- Agents run **fully autonomously** (no permission prompts). Use version control in your projects.

See [project-plan.md](project-plan.md) for the idea and [implementation-plan.md](implementation-plan.md)
for the design.

## Requirements

- **Node.js** 22 or newer and **npm**.
- **Rust**: install [rustup](https://rustup.rs); the version pinned in `rust-toolchain.toml` is
  installed automatically on the first build.
- **At least one agent CLI**, signed in:
  - Claude Code (`claude`). On Windows, Claude Code also needs [Git for Windows](https://git-scm.com).
  - OpenCode (`opencode`) 1.18 or newer.

  Workzone finds them on `PATH`; otherwise set the path in **Settings**, where **Check** tells you
  whether it works.
- **Linux**: the WebKitGTK build dependencies, e.g. on Debian/Ubuntu

  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev
  ```

  (Arch: `webkit2gtk-4.1`). See [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/).
- **Windows**: the "Desktop development with C++" workload of Visual Studio Build Tools. WebView2 is
  part of Windows 10/11.

## Development

```bash
npm install
npm run tauri dev
```

The window reloads on frontend changes; Rust changes rebuild and restart the app. In dev builds the
app state is available as `workzone` in the web inspector's console.

To build a release binary without installers and start it, use the scripts in the repository root:
`build.bat` / `run.bat` on Windows, `./build.sh` / `./run.sh` on Linux. `run` builds first if there
is no binary yet; after code changes, run `build` again.

## Building installers

Run on the platform you are building for (Windows installers can only be built on Windows):

```bash
npm run bundle:linux     # .deb and .AppImage
npm run bundle:windows   # NSIS .exe and .msi
```

The installers end up in `src-tauri/target/release/bundle/`. Building the AppImage downloads Tauri's
`linuxdeploy` helper on first use. There is no auto-update; install a new build over the old one.

App data (database, generated agent config) lives in `~/.local/share/dev.workzone.app` on Linux and
`%APPDATA%\dev.workzone.app` on Windows.

## Checks and tests

```bash
npm run lint                                   # Prettier + svelte-check
npm run build                                  # the Rust build embeds the built frontend
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo test                     # unit and scheduler tests, no agent needed
```

`.github/workflows/ci.yml` runs these checks on Linux and Windows wherever GitHub-style Actions are
available.

The end-to-end tests run the real agent CLIs against a temporary project. They use tokens (with a
cheap or free model they cost cents), so they are ignored by default:

```bash
cd src-tauri && cargo build
WORKZONE_CLAUDE_MODEL=haiku WORKZONE_OPENCODE_MODEL=opencode/big-pickle \
  cargo test agent::e2e -- --ignored --nocapture --test-threads 1
```

Add `claude` or `opencode` to the filter (`agent::e2e::opencode`) to test one agent.

## Environment variables

| Variable | Purpose |
|---|---|
| `WORKZONE_CLAUDE_BIN`, `WORKZONE_OPENCODE_BIN` | Agent executable when none is set in Settings |
| `WORKZONE_CLAUDE_MODEL`, `WORKZONE_OPENCODE_MODEL` | Model when none is set in Settings |
