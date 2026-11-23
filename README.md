<p align="center">
  <img src="Aquilum-logo.png" alt="Apus" width="120">
</p>

<h1 align="center">Apus</h1>

<p align="center">
  A light, fast, local-first notes app.<br>
  Plain Markdown on your disk, a focused editor, instant search, links, a graph, and an infinite canvas.
</p>

<p align="center">
  <a href="#installation">Installation</a>
  · <a href="#whats-different-in-apus">What's different</a>
  · <a href="#license-and-credits">License and credits</a>
  · <a href="README.ru.md">Русский (original project)</a>
</p>

---

## About

Apus is a personal fork of [Aquilum](https://github.com/Freaction/Aquilum) by Dmitriy Chaplinskiy. It is named after *Apus*, the genus of swifts: small, light, quick birds.

It opens a folder you choose and works directly with the `.md` files inside it. There is no account, no proprietary note format and no cloud database, so your notes stay usable in any other tool and you can back them up or sync them however you like.

The interface is native. It is written in Rust on Masonry, winit and vello_cpu, without a WebView or a browser engine, and disk access, indexing and search run in Rust too. The goal is an Obsidian-style workspace that starts fast and stays light on memory.

## What's different in Apus

Compared with upstream Aquilum 0.3.28, this fork adds or changes:

- **Canvas.** An infinite board stored in `.canvas` files, with nodes and edges. Cards can hold live Markdown (the same editor as note pages), images, links to notes and groups. The board remembers its camera position, and only the cards near the view are drawn live.
- **Zoom memory fix.** Zooming on a canvas made memory grow, because the glyph cache keeps one entry per font size. The cache is now released when the camera rests and when you switch tabs.
- **Spanish interface**, next to English and Russian, plus a pass that moved the code's comments, messages and developer docs from Russian to English.
- **Catppuccin themes**, per-level heading styles and theme editing.
- **Its own identity.** Apus installs and runs next to Aquilum, with a separate data folder, startup entry and window class.
- **Updates are off** until this fork publishes its own update manifest. The app will not offer upstream's builds.

Everything else (editor, search, graph, tasks, tables, history, trash, book reader, templates, MCP server) comes from Aquilum. See the upstream README for the full tour of those features.

## Features

### Writing and organization

- Native editor with live Markdown preview and a document outline.
- Reorderable tabs, restored sessions, back and forward navigation.
- File tree with create, rename, move, duplicate and safe delete.
- YAML frontmatter shown as editable fields; searchable templates with date and time placeholders.
- Images and videos stored as local attachments, with cropping in the editor.
- Several independent vaults, each with its own index.
- Note history with edit sources, visual diffs, named versions and per-change revert.
- Trash with configurable retention. External changes are merged safely, and unresolvable ones are kept in a conflict copy.

### Search and discovery

- Full-text vault search while you type, and search inside the current note.
- Backlinks, outgoing links, related-note suggestions and an interactive link graph.
- Live `TABLE`, `LIST` and `TASK` queries over notes and frontmatter.

### Reading and export

- Built-in reader for EPUB, MOBI, AZW3, FB2 and PDF, with reading progress and saved quotes.
- Export a note to an A4 PDF.

### Agents (optional)

A local MCP server lets compatible AI tools search, read and edit notes. It only runs when you turn it on.

## Installation

Apus for Windows is distributed as an installer built from this repository.

1. Run `Apus-<version>-setup.exe`.
2. Choose a new or existing folder for your vault.

The installer is not signed with a Windows code-signing certificate, so SmartScreen may show a warning. Choose **More info → Run anyway** if you got it from someone you trust.

Only the Windows build has been tested for this fork. The code base still contains the macOS, Linux and Android targets inherited from Aquilum, but they have not been built or checked here.

### Build from source

You need Rust (stable) and, on Windows, the MSVC Build Tools with the C++ workload.

```sh
cd aquilum-app/native
cargo run --release          # run it
cargo test                   # run the tests
```

To produce the Windows installer (the `installed` feature adds autostart and the updater):

```powershell
cargo install cargo-packager --version 0.11.8 --locked
cd aquilum-app
cargo build --release -p aquilum-native --features installed
copy ..\.artifacts\cargo-native\release\aquilum-native.exe ..\.artifacts\cargo-native\release\aquilum-app.exe
cargo packager -p aquilum-native --release --formats nsis --out-dir ..\.artifacts\packages
```

More build notes are in [DEVELOPMENT.md](DEVELOPMENT.md) and [aquilum-app/README.md](aquilum-app/README.md).

## Privacy and data locations

- Notes are `.md` files in the vault folder you selected; attachments and books live there too.
- Settings, window state and search indexes live in Apus's application-data folder (`com.sconelli.apus`).
- There is no account. Network access happens only if you explicitly use source analysis (Wikipedia lookups) or enable features that need it. The MCP server is local.

## License and credits

Apus is free software under the [GNU Affero General Public License v3.0 only](LICENSE), the same license as the original project.

- Original work: **Aquilum**, copyright © 2026 Dmitriy Chaplinskiy, <https://github.com/Freaction/Aquilum>.
- Changes in this fork: copyright © 2026 Santiago Conelli, under the same license.

Under the AGPL you may use, study, modify and share this software. If you distribute it, or let others use a modified version over a network, you must provide the complete source code under the same license. The source for the builds of Apus is this repository.

The names "Aquilum" and its logo identify the original project and are not licensed for modified versions, which is why this fork uses a different name and icon. The logo file keeps its old file name (`Aquilum-logo.png`) only to avoid breaking paths; the image is the Apus mark.

Bundled third-party components (in `aquilum-app/native/vendor/` and fonts) keep their own licenses, included next to them.
