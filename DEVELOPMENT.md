# Aquilum

**Aquilum** is an ultra-fast local-first editor for a personal and team knowledge base. The interface is native: **Rust**, **Masonry**, **winit** and **vello_cpu**, with no WebView.

The app is made for those who value maximum performance, privacy and a comfortable way to work with thoughts, texts and projects.

- **Your data belongs to you**: notes are stored in plain `.md` files on your disk, in any folder you like.
- **Conflict-free architecture**: edits are processed in memory through a CRDT (**Yrs**, Yjs in Rust), preserving integrity under concurrent changes.
- **Local speed**: the search engine, analytics and caching are implemented in **Rust** with a local SQLite database.
- **No cloud lock-in**: works fully offline, with no subscriptions and no telemetry.

---

## Stack

| Layer | Technologies |
| --- | --- |
| Language | **Rust** (edition 2024) |
| Window and input | **winit 0.31** (own host, without `masonry_winit`), DirectManipulation for the touchpad on Windows |
| Interface | **Masonry** (Xilem widgets): a fork of `masonry_core` in `aquilum-app/native/vendor/` |
| Text | **Parley** (layout), **harfrust** (shaping), **skrifa** (fonts), iA Writer fonts embedded |
| Rendering | **vello_cpu** on the CPU (SSE4.2), forked in `vendor/` together with **glifo** (glyph atlas); own damage-region tracking, layers and stamps |
| Frame output | Windows: **DXGI** swap chain (flip, WARP), other OSes: **softbuffer** |
| Editor | own: Markdown with live preview, paragraphs are laid out only in the visible area, estimated heights and a scroll anchor like in CodeMirror 6, markup is parsed only around the edit |
| Storage and core | `aquilum-app/core`: **SQLite** (rusqlite), **Yrs** (CRDT), **tantivy** (full-text search), **notify** (file watching), MCP server |
| Images | **image** (PNG, JPEG, WebP, GIF, BMP); PNG and JPEG are decoded directly to the display size (`png`, `jpeg-decoder`) |
| PDF export | **krilla** |
| Network | **ureq** + **rustls** (updates, image downloads, link titles) |
| Installers | **cargo-packager** (NSIS, `.app`, DMG, AppImage) and **nfpm** (deb, rpm) |

---

## Features

### 📝 Live Preview editor
- **Seamless on-the-fly preview**: Markdown formatting (headings, lists, quotes, text styling) is rendered immediately as you type, with no split between code and preview.
- **Interactive tables**: a visual table editor with keyboard navigation (<kbd>Tab</kbd>, <kbd>Enter</kbd>, <kbd>Esc</kbd>) and adding/removing rows and columns through the context menu.
- **Callouts**: support for information blocks `[!note]`, `[!warning]`, `[!quote]`, `[!book]` and others.
- **Wiki links and autocomplete**: a smart system of two-way links `[[Note name|Alias]]` and tags `#tag` with a popup suggestion list.
- **Frontmatter metadata**: a collapsible YAML metadata block with validation and type highlighting.
- **In-page search**: fast find and replace in the open note (<kbd>Ctrl</kbd> + <kbd>F</kbd>) with cyclic navigation through matches.

### 📚 Built-in book reader (EPUB / FB2)
- Read books right inside the app, without opening third-party programs.
- Automatic saving of progress, percentage read and the exact position in the book (CFI).
- **Linked quotes**: selecting a text fragment in a book creates a `> [!quote]` quote block in the note with an `aquilum-reader:` link. Clicking the footnote in the note instantly opens the book at the exact place of the quote.

### 📊 Dataview analytical query engine
- A built-in lightweight and fast table query engine in Rust:
  ```dataview
  TABLE file.mtime AS "Modified", status AS "Status"
  FROM "Projects"
  WHERE status != "done"
  ```
- Support for aggregates and periods: `count`, `delta`, `sum`, date intervals (`span`, `dur`, `today`, `now`).
- **Interactive tasks**: toggling task checkboxes right in Dataview table cells instantly updates the corresponding Markdown file on disk.
- **Compact countdown**: deadline timers that show seconds when less than a minute remains, with a compact completion marker.

### 🕸️ Interactive 2D knowledge graph
- A clear map of the links between all notes of your vault in real time.
- Physical simulation of link elasticity with smooth zoom.
- **Time highlight modes**:
  - `Created`: a color scale by note creation time.
  - `Modified`: highlighting by recent changes.
  - `None`: a concise monochrome mode.
- Focus on the selected note and highlighting of adjacent edges.

### 🔗 Backlinks and semantic analysis
- A right panel with incoming and outgoing mentions of the open note.
- Semantic algorithms for analyzing adjacency and context:
  - **BM25F**: contextual text search with field normalization.
  - **Adamic-Adar**: a graph algorithm for finding common neighbors.
  - **Wikixiv**: topic recommendations based on term analysis.

### 🕒 Local version history (Note History)
- Versions are created automatically in `.aquilum/history/` on edits and external changes.
- Convenient history browsing with a visual diff (added and removed lines are highlighted).
- Naming of versions to pin key checkpoints.
- Quick restore of any version in one click without losing the current state.

### 🤖 Native MCP server (Model Context Protocol)
- A built-in server over HTTP and STDIO for connecting to AI assistants (Cursor, Claude Desktop, Antigravity and others).
- A set of tools for agents:
  - Searching and filtering notes (`search_notes`, `find_notes`, `note_outline`).
  - Reading and safely creating/changing files (`read_note`, `create_note`, `update_note`).
  - Managing links and the graph (`backlinks`, `outgoing_links`, `related_notes`).
  - Working with version history (`note_history`, `read_version`, `restore_version`).
  - **Background guest vaults**: an agent can read and search in any previously opened vault without switching the user's active window.

### 🗑️ Safe trash (.trash)
- Protection against accidental deletion: files and their history are moved to the vault's local trash with the possibility of instant restore.

---

## Quick start

### Requirements
- **Rust**: current stable version (`rustup update stable`)
- **Visual Studio Build Tools** (for Windows): the C++ x64/x86 build tools component
- **Node.js** v20+: only for `npm run dev` (rebuild on the fly); without it, `aquilum-app\native\watch.bat` does the same

### Running for development

```powershell
cd aquilum-app
npm run dev
```

Builds, opens the window and restarts it after each change to the code, core, styles or translations. The data directory is separate: `com.dmitriy.aquilum-app.native-dev`. In the window, F12 turns on repaint highlighting (not available in release).

`aquilum-app\native\run.bat` builds release and opens the window with the regular data.

---

## Checks

```powershell
cd aquilum-app
cargo test --workspace
```

---

## Release build

All packages are built by the `Release` workflow in the public repository (Actions → Release, started manually):

- Windows: the NSIS installer `Aquilum_<version>_x64-setup.exe`;
- macOS: DMG and `.app.tar.gz` archives for auto-update (Apple Silicon and Intel);
- Linux: AppImage, `.deb` and `.rpm`;
- `latest.json` for auto-update; all files are signed with the same minisign key.

Packaging uses [cargo-packager](https://github.com/crabnebula-dev/cargo-packager) (`[package.metadata.packager]` in `aquilum-app/native/Cargo.toml`) and [nfpm](https://nfpm.goreleaser.com) (`aquilum-app/native/packaging/nfpm.yaml`). The installer installs over the previous Tauri version. Before upload, each package is installed and launched on a clean machine; building the Windows installer locally is described in `aquilum-app/README.md`.

---

## Author
- **Developer**: Dmitriy Chaplinskiy
- **Telegram**: [@dmitriy_yiu](https://t.me/dmitriy_yiu)
- **License**: [GNU AGPL-3.0-only](LICENSE). © 2026 Dmitriy Chaplinskiy. Public page, sources and releases: [Freaction/Aquilum](https://github.com/Freaction/Aquilum).
