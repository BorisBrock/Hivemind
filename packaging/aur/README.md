# AUR packages

- `hivemind-ai/` builds the latest tagged release from its GitHub source tarball.
- `hivemind-ai-bin/` repackages the `.deb` of the latest GitHub release, so it
  installs in seconds instead of compiling. x86_64 only, like the release builds.
- `hivemind-ai-git/` builds the latest commit on `main`.

`hivemind-ai` and `hivemind-ai-git` build Hivemind with Tauri's `.deb` bundler
and copy its staged `/usr` tree (binary, desktop file, icons) into the package.
`hivemind-ai-bin` unpacks the same `/usr` tree from the released `.deb`. All
three install the binary as `/usr/bin/hivemind-ai` (set in
`src-tauri/tauri.linux.conf.json`) and can't be installed side by side.

The names have the `-ai` suffix because `hivemind` is taken by an unrelated
process manager: the AUR's `hivemind-bin` installs `/usr/bin/hivemind` and
provides `hivemind`. Up to 1.0.1 these packages were called `hivemind` and
`hivemind-git`; they conflict with `hivemind<=1.0.1` and `hivemind-git` so
pacman offers to remove the old package. The version constraint keeps
`hivemind-bin`'s unversioned `provides` from matching.

Each directory is pushed as-is to its own AUR git repository. The AUR needs
a `.SRCINFO` next to every `PKGBUILD`; regenerate it after each change:

```bash
makepkg --printsrcinfo > .SRCINFO
```

## Releasing a new version

Pushing a stable `vX.Y.Z` tag updates the AUR `hivemind-ai` and
`hivemind-ai-bin` packages automatically: the `aur` job in
`.github/workflows/release.yml` runs once per package, after the GitHub release
is published (`hivemind-ai-bin` downloads its `.deb`). It runs
`update-release.sh`, which takes the package's `PKGBUILD` from this repository,
sets the version and checksums, test-builds it and pushes it to the AUR.
Pre-releases (`v0.x`, `-rc1` and so on) are skipped.

`hivemind-ai/PKGBUILD` and `hivemind-ai-bin/PKGBUILD` here are templates, so
their `pkgver` and checksums stay at the version they were last edited for.
Change them here when the packaging itself changes; the job fills in the
version.

The job logs in with an SSH key of its own, stored as the repository secret
`AUR_SSH_PRIVATE_KEY`. Its public half is one of the keys on the AUR account,
so it can push to every package of that account.

To update by hand instead, for example if the job failed:

```bash
git clone ssh://aur@aur.archlinux.org/hivemind-ai-bin.git ~/aur/hivemind-ai-bin
packaging/aur/update-release.sh hivemind-ai-bin X.Y.Z ~/aur/hivemind-ai-bin
git -C ~/aur/hivemind-ai-bin push
```

The same works for `hivemind-ai`. Cloning a package that doesn't exist on the
AUR yet gives an empty repository; the first push creates the package.

`hivemind-ai-git` needs no update per release; its `pkgver()` follows the
repository. Push it again only when the `PKGBUILD` itself changes.
