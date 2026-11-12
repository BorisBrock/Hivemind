# Building Hivemind

Hivemind is a [Tauri 2](https://v2.tauri.app) app: a SvelteKit + TypeScript frontend in `src/`
and a Rust backend in `src-tauri/`.

## Requirements

- **Node.js** 22 or newer and **npm**.
- **Rust**: install [rustup](https://rustup.rs); the version pinned in `rust-toolchain.toml` is
  installed automatically on the first build.
- **Linux**: the WebKitGTK build dependencies, see
  [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/).

  ```bash
  # Debian / Ubuntu
  sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev
  # Arch / CachyOS
  sudo pacman -S --needed webkit2gtk-4.1 base-devel
  ```

- **Windows**: the "Desktop development with C++" workload of Visual Studio Build Tools. WebView2 is
  part of Windows 10/11.
- To try it out, at least one agent CLI (Claude Code or OpenCode), see the [README](../README.md#-requirements).

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

## Building a release locally

`release.bat` (Windows) and `./release.sh` (Linux) build a release and put it into `.release/`:

| Platform | Files |
|---|---|
| Windows | `Hivemind-<version>-windows-<arch>.exe` (portable, runs without installation) and a `.zip` with the executable and the README |
| Linux | `Hivemind-<version>-linux-<arch>.AppImage` (portable, bundles its libraries) and a `.tar.gz` with the bare binary and the README (needs WebKitGTK installed) |

Pass `--installers` to also build the installers and copy them there (NSIS `.exe` and `.msi` on
Windows, `.deb` on Linux). The version comes from `src-tauri/Cargo.toml`, which
is `0.0.0` in the repository; release builds get theirs from the git tag (run
`tools/set-version.sh 1.2.0` first to build a specific version locally, but don't commit it). Close Hivemind before
building, because the build overwrites `src-tauri/target/release/hivemind(.exe)`.

Only the installers, built on the platform they are for:

```bash
npm run bundle:linux     # .deb and .AppImage
npm run bundle:windows   # NSIS .exe and .msi
```

They end up in `src-tauri/target/release/bundle/`. Building the AppImage downloads Tauri's
`linuxdeploy` helper on first use.

## Checks and tests

```bash
npm run lint                                   # Prettier + svelte-check
npm test                                       # frontend unit tests
npm run build                                  # the Rust build embeds the built frontend
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo test                     # unit and scheduler tests, no agent needed
```

`cargo test` also regenerates `src/lib/bindings.ts` (the typed Tauri commands); CI fails if the
committed file is out of date.

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

## App data

The database, pasted images and generated agent config live in
`~/.local/share/dev.hivemind.app` on Linux and `%APPDATA%\dev.hivemind.app` on Windows.
Hivemind was called Workzone before; on its first start it copies the database and window state
over from `dev.workzone.app` and leaves that folder as a backup.
