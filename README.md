# Hivemind

A desktop board for running AI coding agents across several projects. Write a ticket, move it to
**Ready**, and Hivemind runs [Claude Code](https://claude.com/claude-code) or
[OpenCode](https://opencode.ai) headless in the project folder. You follow the agent's work in the
ticket and answer its questions there, instead of juggling a dozen terminal sessions.

- One agent per project at a time, any number of projects in parallel.
- Tickets move through **Planning → Ready → In Progress → Needs Input → Done** on a Kanban board.
- Paste screenshots (Ctrl+V) into a ticket or reply; the agent receives them as images.
- Files that an agent mentions become links, and images (also SVG) show as previews below its message.
- Message an agent while it works ("What are you doing right now?"). Claude Code reads it at its
  next step; OpenCode gets it as its next turn once the current one ends.
- Agents run **fully autonomously** (no permission prompts). Use version control in your projects.

## Requirements

- **Node.js** 22 or newer and **npm**.
- **Rust**: install [rustup](https://rustup.rs); the version pinned in `rust-toolchain.toml` is
  installed automatically on the first build.
- **At least one agent CLI**, signed in:
  - Claude Code (`claude`). On Windows, Claude Code also needs [Git for Windows](https://git-scm.com).
  - OpenCode (`opencode`) 1.18 or newer.

  Hivemind finds them on `PATH`; otherwise set the path in **Settings**, where **Check** tells you
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
app state is available as `hivemind` in the web inspector's console.

To build a release binary without installers and start it, use the scripts in the repository root:
`build.bat` / `run.bat` on Windows, `./build.sh` / `./run.sh` on Linux. `run` builds first if there
is no binary yet; after code changes, run `build` again.

## Building a release

`release.bat` (Windows) and `./release.sh` (Linux) build a release and put it into `.release/`:

| Platform | Files |
|---|---|
| Windows | `Hivemind-<version>-windows-<arch>.exe` (portable, runs without installation) and a `.zip` with the executable and this README |
| Linux | `Hivemind-<version>-linux-<arch>.AppImage` (portable, bundles its libraries) and a `.tar.gz` with the bare binary and this README (needs WebKitGTK installed) |

Pass `--installers` to also build the installers and copy them there (NSIS `.exe` and `.msi` on
Windows, `.deb` on Linux). The version comes from `src-tauri/tauri.conf.json`. Close Hivemind before
building, because the build overwrites `src-tauri/target/release/hivemind(.exe)`.

## Building installers

Run on the platform you are building for (Windows installers can only be built on Windows):

```bash
npm run bundle:linux     # .deb and .AppImage
npm run bundle:windows   # NSIS .exe and .msi
```

The installers end up in `src-tauri/target/release/bundle/`. Building the AppImage downloads Tauri's
`linuxdeploy` helper on first use. There is no auto-update; install a new build over the old one.

App data (database, generated agent config) lives in `~/.local/share/dev.hivemind.app` on Linux and
`%APPDATA%\dev.hivemind.app` on Windows. Hivemind was called Workzone before; on its first start it
copies the database and window state over from `dev.workzone.app` and leaves that folder as a backup.

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
HIVEMIND_CLAUDE_MODEL=haiku HIVEMIND_OPENCODE_MODEL=opencode/big-pickle \
  cargo test agent::e2e -- --ignored --nocapture --test-threads 1
```

Add `claude` or `opencode` to the filter (`agent::e2e::opencode`) to test one agent.

## Environment variables

| Variable | Purpose |
|---|---|
| `HIVEMIND_CLAUDE_BIN`, `HIVEMIND_OPENCODE_BIN` | Agent executable when none is set in Settings |
| `HIVEMIND_CLAUDE_MODEL`, `HIVEMIND_OPENCODE_MODEL` | Model when none is set in Settings |
