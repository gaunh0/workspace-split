# SpaceBox

A **local-first project workspace for Windows**. Create "spaces", link files and folders from
anywhere on disk, add notes, and search everything with SQLite FTS5 — **without copying or moving a
single file**. The database stores absolute paths only.

SpaceBox is built with Tauri v2 (no Electron), a Svelte 5 frontend, and a Rust backend.
Repository: <https://github.com/gaunh0/workspace-split>.

## Origin and purpose

SpaceBox is an **independent clone of [ClingSpace](https://clingspace.com)**, a commercial
local-first project workspace for Windows. It is not affiliated with or endorsed by ClingSpace.

The project exists for one reason: **to try Rust in a real desktop application**. The goal was to
learn by building the parts that make Rust interesting here — a Tauri v2 backend, `rusqlite` with
FTS5, `notify` file watching through `ReadDirectoryChangesW`, `thiserror`-based error handling, and
Windows path normalization with `dunce`.

It is a learning project, not a drop-in replacement for the original app. Features are implemented
from scratch and intentionally kept small; the UI is a Svelte rewrite rather than a copy.

## Features

- **Virtual grouping** — spaces reference files and folders in place, so nothing is duplicated.
- **Full-text search** — FTS5 index over title, note, and path, with prefix matching and BM25
  ranking (title weighted highest).
- **Missing-file detection** — one command re-stats every linked path and updates `is_missing`;
  locked or permission-denied files are never falsely flagged.
- **Live file watching** — `notify` uses `ReadDirectoryChangesW` on Windows and pushes
  `resources-changed` events to the UI when linked files appear, change, or disappear.
- **Notes and titles** — every resource carries its own editable metadata.
- **Reveal in Explorer** — `explorer.exe /select,<path>` through a Rust command.
- **Secure by default** — a strict CSP and a minimal capability set; arbitrary paths are handled by
  Rust, not by webview JavaScript.

## Stack

| Layer | Choice |
| --- | --- |
| Shell | Tauri v2 (Rust) + WebView2 |
| Frontend | Vite 8, Svelte 5, TypeScript, Tailwind CSS 4 |
| Storage | SQLite via `rusqlite` (bundled), WAL mode, FTS5 |
| Watching | `notify` 8 → `ReadDirectoryChangesW` |
| Target | Windows 10/11, `x86_64-pc-windows-msvc` |

## Prerequisites

1. **Rust stable** via [rustup](https://rustup.rs) with the MSVC toolchain.
2. **Visual Studio Build Tools** with *Desktop development with C++* (MSVC + Windows SDK).
3. **WebView2 Runtime** — preinstalled on Windows 11; the installer can bootstrap it.
4. **Node.js 20.19+ or 22.12+** with npm.

## Quick start

```powershell
git clone https://github.com/gaunh0/workspace-split.git
cd workspace-split
npm install
npm run tauri dev
```

The first Rust build compiles SQLite and Tauri, so it takes a few minutes. Later builds are fast.

## Build

```powershell
npm run tauri build                 # NSIS + MSI installers
npm run tauri build -- --no-bundle  # executable only (faster)
```

Artifacts:

```text
src-tauri/target/release/bundle/nsis/   # .exe installer
src-tauri/target/release/bundle/msi/    # .msi installer
src-tauri/target/release/               # portable executable
```

## Project structure

```text
.
├── .github/workflows/ci.yml        # Windows CI: check, build, fmt, clippy, test, bundle
├── index.html
├── package.json
├── svelte.config.js
├── tsconfig.json
├── vite.config.ts                  # Vite + Svelte + Tailwind 4 (port 1420)
├── rust-toolchain.toml
├── src/                            # frontend
│   ├── App.svelte                  # layout: spaces sidebar, search, resource list
│   ├── app.css                     # Tailwind entry + dark theme
│   ├── main.ts
│   ├── vite-env.d.ts
│   └── lib/
│       ├── api.ts                  # typed invoke() wrappers
│       ├── types.ts                # Space / Resource / MissingReport
│       └── components/
│           ├── ResourceCard.svelte
│           └── Sidebar.svelte
├── static/favicon.png
└── src-tauri/                      # backend
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json
    ├── capabilities/default.json
    ├── icons/
    └── src/
        ├── main.rs                 # Windows subsystem entry point
        ├── lib.rs                  # builder, plugins, DB path, state, command registry
        ├── commands.rs             # Tauri commands
        ├── db.rs                   # migrations, FTS5, CRUD, search
        ├── error.rs                # thiserror AppError, serializable for the frontend
        ├── models.rs               # Space, Resource, ResourceType, MissingReport
        ├── paths.rs                # dunce canonicalization, path storage, probes
        ├── state.rs                # Mutex<Connection> + watcher slot
        └── watcher.rs              # notify watcher and event emission
```

## Where data lives

```text
%APPDATA%\com.gaunh0.spacebox\workspace.db
```

`directories::BaseDirs::data_dir()` resolves to `%APPDATA%` (Roaming) on Windows, and the folder
matches the Tauri bundle identifier. WAL files (`workspace.db-wal`, `-shm`) sit next to it.
Deleting the database never touches the files you linked.

## Schema

```sql
CREATE TABLE spaces (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE resources (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    space_id    INTEGER NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
    type        TEXT    NOT NULL CHECK (type IN ('file', 'folder')),
    path        TEXT    NOT NULL COLLATE NOCASE,  -- canonical, forward slashes
    title       TEXT    NOT NULL,
    note        TEXT    NOT NULL DEFAULT '',
    is_missing  INTEGER NOT NULL DEFAULT 0 CHECK (is_missing IN (0, 1)),
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (space_id, path)
);
```

`resources_fts` is an FTS5 **external-content** table over `(title, note, path)` using
`unicode61 remove_diacritics 2` with prefix indexes. `AFTER INSERT/UPDATE/DELETE` triggers keep it
in sync, so text is indexed once instead of duplicated. Search builds a safe prefix query
(`"term"*` joined with `AND`) so FTS5 operators cannot be injected, and ranks with
`bm25(resources_fts, 10.0, 2.0, 1.0)`.

Migrations are tracked with `PRAGMA user_version`; add `MIGRATION_V2` and bump the version when the
schema changes.

## Tauri commands

| Command | Purpose |
| --- | --- |
| `create_space(name)` | Create a named space |
| `list_spaces()` | List spaces alphabetically |
| `delete_space(spaceId)` | Delete a space and its rows (files untouched) |
| `add_resource(spaceId, path, title?, note?)` | Canonicalize, verify existence, store the path |
| `list_resources(spaceId)` | All resources in a space, missing last |
| `search_resources(query, spaceId?, limit?)` | FTS5 prefix search |
| `update_resource(id, title, note)` | Edit title and note |
| `delete_resource(id)` | Remove a row (does not delete the file) |
| `check_missing_files(spaceId?)` | Re-stat paths, update `is_missing` |
| `reveal_in_explorer(path)` | Reveal the item in Windows Explorer |

## Windows-specific design

- **Canonical paths.** `dunce::canonicalize` resolves real casing and symlinks while stripping the
  `\\?\` verbatim prefix added by `std::fs::canonicalize`. Paths are stored with `/` separators, so
  UNC paths become `//server/share/...` and FTS tokenization stays uniform.
- **De-duplication.** `path` uses `COLLATE NOCASE`, so `C:/Work/a.txt` and `c:/work/a.txt` cannot
  both be added to the same space.
- **Locked files.** `paths::probe` treats `NotFound` as missing, but permission or lock errors as
  `Unknown` — a file held by another process is never flagged incorrectly. Watcher setup failures
  (removed or disconnected folders) are logged and ignored.
- **Watcher.** Watches are derived from the database: parent directories for files (non-recursive),
  folders recursively. On create/remove/rename only the affected resources are re-statted, then the
  frontend receives `resources-changed`.
- **Security.** The `fs` plugin keeps its safe default scope (app-data only); arbitrary paths are
  read by Rust commands. Capabilities grant only `core:default`, `dialog:allow-open`, and
  `fs:default`. Production builds use a strict CSP.

## Checks and CI

```powershell
npm run check
npm run build
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib
```

`.github/workflows/ci.yml` runs the whole list plus `npm run tauri build -- --no-bundle` on
`windows-latest` for every push to `main` and every pull request.

## How this project started

The shell was bootstrapped with the official generator:

```powershell
npm create tauri-app@latest spacebox -- --template svelte-ts --manager npm --identifier com.gaunh0.spacebox --tauri-version 2 --yes
```

The `svelte-ts` template ships SvelteKit; this repository removes SvelteKit and adapter-static in
favour of a plain Vite + Svelte SPA and adds Tailwind CSS 4, which keeps startup time and memory use
lower.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| `failed to run 'cargo metadata' ... program not found` | Add `%USERPROFILE%\.cargo\bin` to your user `PATH`, then open a **new** terminal. |
| `link.exe not found` or `cargo metadata` still fails | Install Visual Studio Build Tools with the *Desktop development with C++* workload (MSVC + Windows SDK). |
| `The bundle identifier ... must contain only alphanumeric characters ...` | Tauri identifiers cannot contain `_`. Use hyphens, e.g. `com.example.my-app`. |
| Port 1420 already in use | Stop the previous `npm run tauri dev` (or the leftover `spacebox.exe`) and retry. |
| Window opens but files are not flagged missing | Press the refresh/check action in the UI; the watcher only reports changes while the app is running. |
