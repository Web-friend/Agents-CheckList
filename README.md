# Checklister

Checklister is a Rust checklist CLI backed by a persistent SQLite database. A JavaScript MCP server exposes the same operations to MCP clients by invoking the CLI. The Rust library remains the source of the checklist behavior.

## CLI usage

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

CLI commands print JSON to stdout. Errors print JSON to stderr and return a non-zero exit code. The database defaults to `checklister.db` in the current directory. Set `DATABASE_URL` to use another SQLite file.

## MCP tools

The MCP server uses the official TypeScript SDK v2 over stdio and exposes:

- `create_checklist`, `list_checklists`, `show_checklist`, `delete_checklist`
- `add_check`, `list_checks`, `list_undone_checks`, `complete_check`, `undo_check`

Tool responses contain the CLI's JSON output. The server writes diagnostics to stderr; stdout is reserved for MCP messages.

### Run locally

```sh
cargo build --release --bin checklister
npm --prefix mcp ci --omit=dev
DATABASE_URL=sqlite:///absolute/path/checklister.db \
CHECKLISTER_BIN="$PWD/target/release/checklister" \
npm --prefix mcp start
```

Register the command with an MCP host as a stdio server. The database path must be writable by the process.

### Run in Docker

Build the image:

```sh
docker build -t checklister-mcp:local .
```

Configure an MCP host to launch it with stdin attached and a persistent named volume:

```json
{
  "mcpServers": {
    "checklister": {
      "command": "docker",
      "args": [
        "run",
        "--rm",
        "-i",
        "--mount",
        "type=volume,source=checklister-data,target=/data",
        "checklister-mcp:local"
      ]
    }
  }
}
```

The image stores the database at `/data/checklister.db`; the named volume preserves it across container restarts. The container uses stdio and does not expose a network port.

## Database and migrations

- `checklists`: auto-incrementing `id` and `name`.
- `checks`: auto-incrementing `id`, parent `checklist_id`, boolean `status` (default `false`), `title`, text `description`, and `created_at`.

Check order is calculated from creation timestamps, with the check ID as a stable tie-breaker. Deleting a checklist cascades to its checks. Numbered SQL migrations in `migrations/` run when the CLI first opens the database; older inline-schema databases are adopted and their existing check order is preserved.

## Tests

```sh
cargo test
npm --prefix mcp test
```

Both suites use temporary, file-backed SQLite databases.
