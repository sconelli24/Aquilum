# Aquilum

Native app: Rust, Masonry, winit and vello_cpu, with no WebView. Window code is in `native/`, the shared core (files, search, history, MCP) is in `core/`.

## Development

```powershell
native\run.bat
```

Builds release and launches the window. If the window is already open, it is closed normally, with saving, and the new build is launched.

Rebuild on the fly while editing code: `native\scripts\watch.ps1`.

## Checks

```powershell
cargo test -p aquilum-native
cargo test -p aquilum-core
```

## Release

Packages are built by the `Release` workflow in the public repository (started manually). The build uses the `installed` feature (auto-update, autostart); packaging:
- [cargo-packager](https://github.com/crabnebula-dev/cargo-packager) using `[package.metadata.packager]` in `native/Cargo.toml`: NSIS installer, `.app` and DMG, AppImage;
- [nfpm](https://nfpm.goreleaser.com) using `native/packaging/nfpm.yaml`: `.deb` and `.rpm` with an explicit dependency list.

The installer installs to the same place as the Tauri version, with the same uninstall registry key, and updates it; the deb and rpm package names (`aquilum`) are unchanged. Signing uses the same minisign key, and `latest.json` keeps its old format. Before upload, each package is installed and launched on a clean machine.

Windows installer locally, unsigned:

```powershell
cargo install cargo-packager --version 0.11.8 --locked
cargo build --release -p aquilum-native --features installed
copy ..\.artifacts\cargo-native\release\aquilum-native.exe ..\.artifacts\cargo-native\release\aquilum-app.exe
cargo packager -p aquilum-native --release --formats nsis --out-dir ..\.artifacts\packages
```

The app version is set in `core/Cargo.toml` and `native/Cargo.toml` (the same in both).

Build cache: `../.artifacts/cargo-native/`.

The previous Tauri version is in `../tauri-version/` and is needed only to compare features.
