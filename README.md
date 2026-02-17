# Memo-Tori GTK

Native Linux desktop app for ultra-fast thought capture.

## Stack

- Rust
- GTK 4 (without mandatory libadwaita dependency)
- SQLite (FTS5)

## Current Scope

This initial scaffold includes:

- XDG-compliant path resolution
- Config file bootstrap (`~/.config/memo-tori/config.toml`)
- SQLite database bootstrap (`~/.local/share/memo-tori/memo-tori.db`)
- MVP schema + FTS5 virtual table
- Quick capture GTK window with save/cancel actions
- `--version` CLI flag

## Build

```bash
cargo run
```

## XFCE app icon and launcher

Install desktop integration for your local user:

```bash
./scripts/install-local.sh
```

Then log out/in (or restart XFCE panel) if the launcher icon does not refresh immediately.

## Debian package (.deb)

Build an installable Debian package:

```bash
./scripts/build-deb.sh
```

The package is generated in `dist/`.

Install it with:

```bash
sudo dpkg -i dist/memo-tori-gtk_*.deb
```

## Versioning

- SemVer (`MAJOR.MINOR.PATCH`)
- Single source of truth: `Cargo.toml`
- `cargo run -- --version` prints the same version used for builds/packages

## Automatic releases

Releases are automated with Release Please (`.github/workflows/release.yml`):

- Pushes to `main` create/update a release PR based on Conventional Commits.
- When the release PR is merged, version/tag/changelog are updated automatically.
- A `.deb` package is built in CI and attached to the GitHub Release.

Use commit prefixes such as:

- `fix:` for patch releases
- `feat:` for minor releases
- `feat!:` or `BREAKING CHANGE:` for major releases

## Capture hints customization

You can customize random capture hints in `~/.config/memo-tori/config.toml`:

```toml
quit_on_close = false
text_scale = 1.0
capture_hints = [
  "L'idee que je viens d'avoir :",
  "Nouvelle piste :",
  "A garder pour plus tard :"
]
```

Version:

```bash
cargo run -- --version
```
