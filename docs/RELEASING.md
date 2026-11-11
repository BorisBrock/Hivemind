# Releasing Hivemind

Releases are built and published by GitHub Actions.

| Workflow | Runs on | Does |
|---|---|---|
| [`ci.yml`](../.github/workflows/ci.yml) | every push to `main` and every pull request | Prettier, svelte-check, frontend and Rust tests, clippy, and a build on Linux and Windows |
| [`build.yml`](../.github/workflows/build.yml) | the same, plus by hand and from `release.yml` | builds all packages and keeps them as workflow artifacts |
| [`release.yml`](../.github/workflows/release.yml) | tags like `v1.2.0` | builds all packages, publishes the GitHub release and updates the AUR package |

## Publishing a release

1. Set the version (updates `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
   and the lock files):

   ```bash
   tools/set-version.sh 1.2.0
   ```

2. Commit, tag and push:

   ```bash
   git commit -am "Release 1.2.0"
   git tag v1.2.0
   git push origin main v1.2.0
   ```

The release workflow checks that the tag matches the version in `tauri.conf.json`, builds the
packages on Windows and Ubuntu 24.04 and attaches them to a GitHub release named `1.2.0`, with
release notes generated from the merged pull requests and commits.

| Platform | Files |
|---|---|
| Windows | `Hivemind_<version>_x64-setup.exe` (installer), `Hivemind_<version>_x64_en-US.msi`, `Hivemind-<version>-windows-x64.exe` (portable) and `.zip` |
| Linux | `Hivemind-<version>-linux-x64.AppImage`, `Hivemind_<version>_amd64.deb`, `Hivemind-<version>-linux-x64.tar.gz` |

Tags like `v1.2.0-rc1` and all `v0.x` versions become pre-releases and do not update the AUR.

## AUR

Stable releases also update the [`hivemind`](https://aur.archlinux.org/packages/hivemind) AUR
package, which builds from the release's source tarball. The job needs the repository secret
`AUR_SSH_PRIVATE_KEY`. See [`packaging/aur/README.md`](../packaging/aur/README.md) for details and
for updating the package by hand.
