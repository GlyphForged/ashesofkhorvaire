# Ashes of Khorvaire Wiki

A local-first, server-rendered campaign wiki for running **Ashes of Khorvaire**. It uses Rust, Axum, Askama, SQLite/FTS5, HTMX, and the vendored Pell editor. There is no frontend build step and no internet connection is required at runtime.

## Requirements

- Rust stable (install with [rustup](https://rustup.rs/))
- On Windows, Visual Studio C++ Build Tools for the MSVC linker

## Run locally

```powershell
Copy-Item .env.example .env
$env:DATABASE_URL = "sqlite://ashes.db"
$env:BIND_ADDRESS = "127.0.0.1:3000"
cargo run
```

Open <http://127.0.0.1:3000>. The app creates the database, runs embedded migrations, and adds the campaign seed on first startup. Configuration defaults to the values above, so only `cargo run` is normally required.

Environment variables:

| Variable | Default | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | `sqlite://ashes.db` | SQLite connection URL |
| `BIND_ADDRESS` | `127.0.0.1:3000` | Listening address |
| `RUST_LOG` | `ashes_wiki=info,tower_http=info` | Log filtering |

## Editing and links

The page editor has synchronized visual and raw-HTML modes. Type `[[Page Title]]` to create a wiki link. Existing page titles become links when displayed; missing titles lead to a prefilled creation form. Changing a slug preserves the old URL as a permanent redirect.

Every save creates an immutable revision. Archiving removes a page from normal lists, search, and wiki-link resolution while preserving its history. Restore archived pages from **Archive**, or inspect and restore revisions through **History**.

## Test and quality checks

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Integration tests use isolated temporary SQLite databases and cover seeding, FTS search, CRUD revisions, archives, redirects, wiki links, and HTTP responses.

## Backup and deployment

Stop the app and copy `ashes.db` (plus any `-wal`/`-shm` files if present), or use SQLite's online backup command while it is running. For a server deployment, place the app behind a TLS reverse proxy, set an absolute database URL, and use a service manager.

For an Orange Pi running DietPi, follow the complete native ARM build, systemd, Caddy, and backup guide in [`docs/deploy-dietpi.md`](docs/deploy-dietpi.md). Deployment assets live in `deploy/`.

> **Security warning:** `GM SECRET` is a prominent label, not access control. This milestone trusts all locally-authored HTML and has no authentication. Keep the default localhost bind. Do not expose the app publicly until authentication, authorization, CSRF protection, and an HTML sanitization policy are added.

## Project map

- `migrations/` — schema, FTS index, and triggers
- `src/db.rs` — persistence, revisions, archive, search, and idempotent seed
- `src/render.rs` — render-time wiki-link resolution
- `src/web.rs` — routes and server-rendered responses
- `templates/` — Askama HTML templates
- `static/` — dossier CSS plus locally vendored HTMX/Pell assets

HTMX and Pell are distributed under their included license files in `static/vendor/`.
