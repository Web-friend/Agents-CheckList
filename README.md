# Checklister

A command-line checklist manager backed by SQLite. The application operations live in a reusable library layer, so a future MCP server can expose the same operations without duplicating database logic.

## Usage

```sh
cargo run -- checklist create "Release prep"
cargo run -- checklist list
cargo run -- checklist show 1
cargo run -- check add 1 --title "Run checks" --description "Run the project checks before release."
cargo run -- check list 1
cargo run -- check undone 1
cargo run -- check complete 1 1
cargo run -- check undo 1 1
cargo run -- checklist delete 1
```

Commands print JSON to stdout. Errors print JSON to stderr and return a non-zero exit code. Check IDs are auto-incrementing; `position` records their order within a checklist.

The database defaults to `checklister.db` in the current directory. Set `DATABASE_URL` to use another SQLite database, for example `sqlite://data/checklister.db` (the parent directory must already exist).

## Data model

- `checklists`: auto-incrementing `id` and `name`.
- `checks`: auto-incrementing `id`, parent `checklist_id`, boolean `status` (default `false`), `title`, text `description`, and checklist `position`.

Deleting a checklist cascades to its checks. Startup migrates data from the earlier checklist schema.
