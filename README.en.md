# PonyClean

A minimal desktop widget for Windows. Three core features:

- **Process Monitoring** — Real-time detection of processes with abnormally high CPU/memory usage, with alerts and one-click kill; supports calling `EmptyWorkingSet` on non-critical processes to trim memory (releases only the working set, without killing the process)
- **C-Drive Safe Cleanup** — Tiered scanning of cleanable files (temp files, caches, logs, prefetch, old installer leftovers, app caches, dev-tool caches, Windows Update caches, etc.); safely removes them and supports deferred deletion of in-use files
- **Startup Management** — Enumerate all third-party apps that auto-launch on boot (Registry `Run` keys / Startup folders), with one-click disable or re-enable. Windows system startup items are automatically filtered out.

## Feature Highlights

- **Disk Analysis** — Large-file scanning (classified by type) and directory space-usage analysis to locate space hogs
- **Dynamic Island Floating UI** — Dual forms: capsule / edge-docked progress bar, SWCA Acrylic glass, tray icon, system notifications
- **Settings Panel** — Alert thresholds, startup toggle, cleanup-target enable/disable, custom cleanup targets, disk-analysis parameters

## Download & Install

Download `PonyClean_<version>_<arch>-setup.exe` from the [Releases](https://cnb.cool/lanhui100/pony_clean/-/releases) page (`x64` for 64-bit, `arm64` for Windows on ARM), then double-click to install.

> Current builds are **not CA-signed** (self-signed) for internal testing, so the browser / SmartScreen may warn. Follow these steps:
>
> 1. When the browser asks "Keep / Keep anyway" for the `.exe`, click **Keep** (or right-click → Properties → check "Unblock")
> 2. When SmartScreen shows "Windows protected your PC", click **More info → Run anyway**
> 3. Internal testers may double-click `ponyclean-selfsigned.cer` → Install Certificate → "Trusted Root Certification Authorities" to remove the red block
>
> The warning root cause is the unsigned installer; a CA code-signing certificate will be adopted for official releases. Signing scripts live in `scripts/sign/` — see [docs/RELEASE_BUILD.md](docs/RELEASE_BUILD.md).

## Tech Stack

| Layer | Choice |
|---|---|
| Desktop framework | Tauri 2 |
| Frontend | Vue 3 + TypeScript + shadcn-vue |
| Styling | TailwindCSS 4 + Vite |
| Backend | Rust (pony_core business library, zero Tauri dependency) |
| Async | tokio + tokio-util |
| Process | sysinfo |
| Disk | jwalk + windows-rs |

## Development

```bash
npm install             # Install frontend dependencies
npm run dev:tauri       # Tauri dev (frontend HMR + Rust hot reload)
cd frontend && npm run dev   # Start only the Vite dev server
cargo test -p pony_core       # Run unit tests
cargo check -p pony_core -p pony_clean   # Type-check both crates
cargo clippy -p pony_core -p pony_clean  # Clippy lint both crates
```

## Version Management

- Single source of truth for the version: `Cargo.toml` / `frontend/package.json` / `src-tauri/tauri.conf.json` / `Cargo.lock` must all match (enforced by CI)
- Release: `node scripts/bump-version.mjs 0.2.0 [--commit] [--tag]` (syncs version + Cargo.lock + archives CHANGELOG)
- Self-check: `node scripts/check-version.mjs`
- See [docs/VERSIONING.md](docs/VERSIONING.md) for details

## Project Structure

```
pony_clean/
├── crates/pony_core/   # Business core library (pure Rust, framework-agnostic, with unit tests)
│   └── src/
│       ├── monitor.rs  # Process monitoring: snapshot polling + kill + memory trim
│       ├── cleaner.rs  # C-drive cleanup: jwalk traversal + safety tiers + deletion
│       ├── disk.rs     # Disk analysis: large-file scan + directory space usage
│       ├── memory.rs   # Memory trim (EmptyWorkingSet)
│       ├── startup.rs  # Startup management
│       ├── icon.rs     # Process icon extraction (Windows)
│       └── error.rs    # Unified error type
├── src-tauri/          # Tauri shell (command layer + window/tray/glass)
│   └── src/commands/   # Tauri command layer (monitor / cleaner / disk / config / startup / window)
├── frontend/           # Vue 3 frontend (Views: Monitor / Space / Settings / Startup)
├── scripts/            # Version scripts (bump / check + contract tests)
├── docs/               # Project docs
├── CHANGELOG.md        # Changelog
├── 00_DASHBOARD.md     # Task dashboard
├── 01_TASK_BOARD.md    # Task board
└── 03_TASKS/           # Task cards
```

## Documentation

- [Docs Index](docs/README.md)
- [Architecture](docs/ARCHITECTURE.md) — module dependencies, data-flow diagram
- [Design Decisions](docs/DESIGN.md) — ADR technology choices
- [C-Drive Cleanup Strategy](docs/CLEAN_STRATEGY.md) — 60+ cleanup targets + safety strategy
