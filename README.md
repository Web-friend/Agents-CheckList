# Checklister

A Tokio-based TCP echo server with an SQLite database initialized for checklists and their checks. The database layer is kept separate from the TCP server so it can be reused when API routes are added.

The `checklists` table contains an auto-incrementing `id` and a `name`. The `checks` table contains an auto-incrementing `id`, its parent `checklist_id`, a boolean `status`, a `title`, and a `description` stored as text. Deleting a checklist cascades to its checks.

## Run

```sh
cargo run
```

The server listens on `127.0.0.1:8080` by default and creates `checklister.db` in the current directory. Set `CHECKLISTER_ADDR` to change the listen address, or `DATABASE_URL` to choose another SQLite database, for example `sqlite://data/checklister.db`.
