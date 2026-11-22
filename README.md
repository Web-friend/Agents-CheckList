# Checklister

A Tokio-based TCP echo server with an SQLite database initialized for checklist and checklist-item data. The database layer is kept separate from the TCP server so it can be reused when API routes are added.

## Run

```sh
cargo run
```

The server listens on `127.0.0.1:8080` by default and creates `checklister.db` in the current directory. Set `CHECKLISTER_ADDR` to change the listen address, or `DATABASE_URL` to choose another SQLite database, for example `sqlite://data/checklister.db`.
