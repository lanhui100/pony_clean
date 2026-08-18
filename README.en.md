# PonyClean

A minimal desktop widget for Windows. Three core features:

- **Process Monitoring** — Real-time detection of processes with abnormally high CPU/memory usage, with alerts and one-click kill
- **C-Drive Safe Cleanup** — Tiered scanning of cleanable files; safely remove temp files, caches, Recycle Bin, etc.
- **Startup Management** — Enumerate all third-party apps that auto-launch on boot (Registry `Run` keys / Startup folders), with one-click disable or re-enable. Windows system startup items are automatically filtered out.

## Tech Stack

| Layer | Choice |
|---|---|
| Desktop framework | Tauri 2 |
| Frontend | Vue 3 + TypeScript + shadcn-vue |
| Styling | TailwindCSS 4 |
| Backend | Rust (pony_core) |
| Async | tokio |
| Process | sysinfo |
| Disk | jwalk + windows-rs |

## Development

```bash
npm run dev:tauri       # Tauri dev (frontend HMR + Rust hot reload)
cargo test -p pony_core  # Run unit tests
```

## Version Management

- Single source of truth for the version: `Cargo.toml` / `frontend/package.json` / `src-tauri/tauri.conf.json` / `Cargo.lock` must all match (enforced by CI)
- Release: `node scripts/bump-version.mjs 0.2.0 [--commit] [--tag]` (syncs version + Cargo.lock + archives CHANGELOG)
- Self-check: `node scripts/check-version.mjs`
- See [docs/VERSIONING.md](docs/VERSIONING.md) for details

## Project Structure

```
pony_clean/
├── crates/pony_core/  # Business core library (framework-agnostic)
├── src-tauri/         # Tauri shell
├── frontend/          # Vue 3 frontend
├── scripts/           # Version scripts (bump / check + contract tests)
├── docs/              # Project docs
├── CHANGELOG.md       # Changelog
├── 00_DASHBOARD.md    # Task dashboard
├── 01_TASK_BOARD.md   # Task board
└── 03_TASKS/          # Task cards
```
