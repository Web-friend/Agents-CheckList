# Checklister

Checklister is a fully native Rust checklist application backed by a persistent SQLite database. The Rust CLI and the Rust MCP server share the same library, store, storage layer, schema, and migrations.

- **CLI** — `checklister` for manual use and scripts.
- **MCP server** — `checklister-mcp` for AI agents such as OpenCode, Claude Desktop, Codex, Cursor, and Zed.
- **Shared library** — `src/store.rs` and `src/storage.rs` hold the only checklist implementation.

No Node.js, N-API, FFI, or JavaScript layer is involved at build time or runtime.

## Contents

- [Quick start](#quick-start)
- [CLI usage](#cli-usage)
- [Using the MCP server with an AI agent](#using-the-mcp-server-with-an-ai-agent)
  - [How it works](#how-it-works)
  - [Requirements](#requirements)
  - [Build the server](#build-the-server)
  - [Choose a database](#choose-a-database)
  - [Connect an agent](#connect-an-agent)
  - [Verify the connection](#verify-the-connection)
  - [Tool reference](#tool-reference)
  - [Response shapes](#response-shapes)
  - [Errors](#errors)
  - [Permissions and safety](#permissions-and-safety)
  - [Environment variables](#environment-variables)
  - [Troubleshooting](#troubleshooting)
  - [Running the server by hand](#running-the-server-by-hand)
- [Docker](#docker)
- [Development](#development)
- [Database and migrations](#database-and-migrations)
- [Project layout](#project-layout)

## Quick start

Use the CLI directly:

```sh
cargo run -- checklist create "Release prep"
cargo run -- check add 1 --title "Run checks" --description "Run the project checks before release."
cargo run -- checklist show 1
```

Build the native MCP server:

```sh
cargo build --release --bin checklister-mcp
```

Then point your agent at the compiled executable with an absolute database path:

```sh
DATABASE_URL=sqlite:///absolute/path/to/checklister.db \
  /absolute/path/to/target/release/checklister-mcp
```

The agent discovers nine tools and can create checklists, add checks, list remaining work, complete or reopen checks, and delete checklists. See [Connect an agent](#connect-an-agent) for host-specific configuration.

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

## Using the MCP server with an AI agent

### How it works

```text
┌─────────────┐   MCP / JSON-RPC 2.0   ┌────────────────────┐   Rust calls   ┌──────────────────┐   SQL   ┌────────────┐
│  AI agent   │ ◄────────────────────► │  checklister-mcp   │ ◄────────────► │  Store + Storage │ ◄─────► │  SQLite    │
│  (MCP host) │      stdio transport   │  native executable │                │  (shared crate)  │         │  database  │
└─────────────┘                        └────────────────────┘                └──────────────────┘         └────────────┘
```

- The agent launches `checklister-mcp` as a child process and speaks MCP over stdin/stdout.
- The server uses the official Rust MCP SDK (`rmcp`) with its native server abstractions and stdio transport.
- Tool handlers call the shared Rust `Store` directly. No CLI subprocess is spawned per tool call.
- The database is opened once at startup and migrations run before the server begins serving.
- Rust errors such as `checklist not found` are returned as MCP tool errors.
- Diagnostics go to stderr. stdout is reserved exclusively for MCP protocol messages.

The CLI and the MCP server can use the same database file. SQLite serializes writers and Checklister keeps the existing five-second busy timeout.

### Requirements

| Requirement | Needed for | Notes |
| --- | --- | --- |
| Rust 1.88 or newer | Building the CLI and MCP server | The MCP SDK requires Rust 1.88+. |
| A writable database path | Running either binary | The parent directory must exist. The file is created if missing. |
| An MCP-capable agent | Using the tools | OpenCode, Claude Desktop, Codex, Cursor, Zed, or any stdio MCP host. |

Node.js is not required.

### Build the server

```sh
cargo build --release --bin checklister-mcp
```

The executable is written to `target/release/checklister-mcp` (`.exe` on Windows). Copy it anywhere and launch it directly.

For development:

```sh
cargo build --bin checklister-mcp
```

### Choose a database

`DATABASE_URL` controls which SQLite file is used. Absolute paths are strongly recommended, because the working directory differs between the CLI and an agent-launched process.

```sh
# Absolute path (three slashes after sqlite:)
DATABASE_URL=sqlite:///absolute/path/to/checklister.db

# Relative path (resolved from the process working directory)
DATABASE_URL=sqlite://checklister.db
```

When `DATABASE_URL` is unset, the server uses `sqlite://checklister.db` relative to its working directory. Prefer an absolute path.

Create the parent directory before starting the server:

```sh
mkdir -p /absolute/path/to/data
```

SQLite creates the database file and the `-wal` and `-shm` files on first use. Migrations run automatically. Existing databases created by earlier Checklister versions are adopted and preserved.

### Connect an agent

Every MCP host needs the compiled executable and the `DATABASE_URL` environment variable. Use absolute paths everywhere.

#### OpenCode (V2)

OpenCode V2 configures MCP servers under `mcp.servers`. Add this to `opencode.jsonc` in the project, or to the global config at `~/.config/opencode/opencode.jsonc`:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "servers": {
      "checklister": {
        "type": "local",
        "command": ["/absolute/path/to/checklister-mcp"],
        "environment": {
          "DATABASE_URL": "sqlite:///absolute/path/to/checklister.db"
        }
      }
    }
  }
}
```

The CLI can write this entry for you:

```sh
opencode mcp add checklister \
  --env DATABASE_URL=sqlite:///absolute/path/to/checklister.db \
  -- /absolute/path/to/checklister-mcp

opencode mcp list
```

OpenCode reloads configuration after the file is saved; no restart is required. Use `/mcps` to inspect the connection, connect, or disconnect the server.

#### Claude Desktop

Edit the Claude Desktop configuration file:

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "checklister": {
      "command": "/absolute/path/to/checklister-mcp",
      "args": [],
      "env": {
        "DATABASE_URL": "sqlite:///absolute/path/to/checklister.db"
      }
    }
  }
}
```

Restart Claude Desktop after editing the file.

#### Other MCP hosts

Most hosts accept the same `mcpServers` shape:

```json
{
  "mcpServers": {
    "checklister": {
      "command": "/absolute/path/to/checklister-mcp",
      "args": [],
      "env": {
        "DATABASE_URL": "sqlite:///absolute/path/to/checklister.db"
      }
    }
  }
}
```

Some hosts use a single `command` array instead of separate `command` and `args`, and some call the environment map `environment` or `env`. The values are the same:

```text
/absolute/path/to/checklister-mcp
DATABASE_URL=sqlite:///absolute/path/to/checklister.db
```

The server uses the stdio transport. It does not open a network port and does not need a URL, API key, or OAuth configuration.

### Verify the connection

For OpenCode:

```sh
opencode mcp list
```

A healthy entry shows `connected`. Inside OpenCode, `/mcps` shows the same state.

For any host, ask the agent a read-only question first:

```text
List my checklists.
```

That should call `list_checklists` and return either an empty array or the existing checklists. Then try a write:

```text
Create a checklist called "Agent smoke test" and add a check titled "Confirm MCP works".
```

If the tools do not appear, see [Troubleshooting](#troubleshooting).

### Tool reference

All tools take JSON arguments. IDs are positive integers. `check_id` must belong to `checklist_id`; the store scopes every check operation to its parent checklist.

| Tool | Arguments | Returns | Hints |
| --- | --- | --- | --- |
| `create_checklist` | `name` | `{success, message, result: {id, name}}` | Writes; not idempotent |
| `list_checklists` | none | `[{id, name}]` | Read-only; idempotent |
| `show_checklist` | `checklist_id` | `{checklist: {id, name}, checks: [Check]}` | Read-only; idempotent |
| `delete_checklist` | `checklist_id` | `{success, message, result: {id, name}}` | Destructive; cascades to checks |
| `add_check` | `checklist_id`, `title`, `description` | `{success, message, result: Check}` | Writes; not idempotent |
| `list_checks` | `checklist_id` | `[Check]` | Read-only; idempotent |
| `list_undone_checks` | `checklist_id` | `[Check]` | Read-only; idempotent |
| `complete_check` | `checklist_id`, `check_id` | `{success, message, result: Check}` | Writes; idempotent |
| `undo_check` | `checklist_id`, `check_id` | `{success, message, result: Check}` | Writes; idempotent |

A `Check` object has this shape:

```json
{
  "id": 3,
  "checklist_id": 1,
  "status": false,
  "title": "Run tests",
  "description": "Run cargo test.",
  "created_at": "2026-11-22T16:29:09.651Z",
  "order": 3
}
```

`status` is `false` for undone and `true` for complete. `order` is one-based within the checklist and follows creation order. `created_at` is an ISO 8601 UTC timestamp.

#### `create_checklist`

Creates a new, empty checklist.

- **Arguments:** `name` — non-empty display name.
- **Returns:** the generated `id` and the stored `name`.
- **Notes:** every call creates a new checklist. Names are not unique, so use `list_checklists` to disambiguate.

```json
{ "name": "Release 1.0" }
```

#### `list_checklists`

Lists every checklist, ordered by `id`.

- **Arguments:** none.
- **Returns:** an array of `{id, name}`. An empty array means no checklists exist.
- **Notes:** use this first when the agent needs a `checklist_id`.

#### `show_checklist`

Shows one checklist and all of its checks, including completed ones.

- **Arguments:** `checklist_id`.
- **Returns:** `{checklist, checks}`. `checks` is in creation order.
- **Notes:** fails with `checklist not found` if the ID does not exist. Use `list_undone_checks` when the agent only needs remaining work.

```json
{ "checklist_id": 1 }
```

#### `delete_checklist`

Permanently deletes a checklist and every check it contains.

- **Arguments:** `checklist_id`.
- **Returns:** the deleted checklist.
- **Notes:** this cannot be undone. The database cascades the delete to child checks. Fails if the checklist does not exist. Ask the user to confirm before calling this when deletion is not clearly intended.

#### `add_check`

Adds a check to an existing checklist.

- **Arguments:**
  - `checklist_id` — parent checklist.
  - `title` — non-empty short title.
  - `description` — detailed text; use an empty string when there is none.
- **Returns:** the created check with `status: false` and the next `order`.
- **Notes:** fails if the checklist does not exist. Adding the same title twice creates two separate checks.

```json
{
  "checklist_id": 1,
  "title": "Run tests",
  "description": "Run cargo test."
}
```

#### `list_checks`

Lists all checks in a checklist, both complete and undone, in creation order.

- **Arguments:** `checklist_id`.
- **Returns:** an array of checks, or an empty array.
- **Notes:** fails if the checklist does not exist.

#### `list_undone_checks`

Lists only the still-undone checks in a checklist, in creation order.

- **Arguments:** `checklist_id`.
- **Returns:** an array of checks with `status: false`, or an empty array.
- **Notes:** the best tool for "what is left?" questions. Fails if the checklist does not exist.

#### `complete_check`

Marks a check complete.

- **Arguments:** `checklist_id`, `check_id`.
- **Returns:** the updated check with `status: true`.
- **Notes:** completing an already-complete check succeeds without changing it. Fails if the checklist or check does not exist.

```json
{ "checklist_id": 1, "check_id": 3 }
```

#### `undo_check`

Reopens a completed check.

- **Arguments:** `checklist_id`, `check_id`.
- **Returns:** the updated check with `status: false`.
- **Notes:** undoing an already-undone check succeeds without changing it. Fails if the checklist or check does not exist.

### Response shapes

Mutating tools return an envelope:

```json
{
  "success": true,
  "message": "Checklist created",
  "result": {
    "id": 1,
    "name": "Release 1.0"
  }
}
```

Read tools return their data directly:

```json
[
  { "id": 1, "name": "Release 1.0" },
  { "id": 2, "name": "Post-release cleanup" }
]
```

`show_checklist` returns both parts:

```json
{
  "checklist": { "id": 1, "name": "Release 1.0" },
  "checks": [
    {
      "id": 1,
      "checklist_id": 1,
      "status": true,
      "title": "Run tests",
      "description": "Run cargo test.",
      "created_at": "2026-11-22T16:29:09.651Z",
      "order": 1
    }
  ]
}
```

MCP wraps the JSON in a text content block. Agents that parse tool output receive the same objects shown above.

### Errors

Expected failures are returned as tool errors rather than crashing the server:

```json
{
  "success": false,
  "error": "checklist not found"
}
```

Common errors:

| Error | Meaning | Recovery |
| --- | --- | --- |
| `checklist not found` | The `checklist_id` does not exist. | Call `list_checklists` and use a current ID. |
| `check not found` | The `check_id` does not exist in that checklist. | Call `list_checks` or `show_checklist` and use a current ID. |
| `unable to open database file` | The parent directory is missing or not writable. | Create the directory and fix permissions. |
| `database is locked` | Another writer held the database longer than the busy timeout. | Retry the call; avoid long concurrent writes from the CLI and the agent. |

Invalid arguments, such as a non-positive ID or an empty checklist name, are rejected by the tool input schema and parameter validation before the Rust store is called.

### Permissions and safety

The tools publish MCP annotations so agents can make better decisions:

- `list_checklists`, `show_checklist`, `list_checks`, and `list_undone_checks` are marked `readOnlyHint: true`.
- `delete_checklist` is marked `destructiveHint: true`.
- `create_checklist` and `add_check` are not idempotent, because repeating them creates more data.
- `complete_check` and `undo_check` are idempotent.

Annotations are hints, not enforcement. If you want hard limits, configure permissions in the MCP host. For OpenCode, deny deletion while leaving the rest available:

```jsonc
{
  "permissions": [
    {
      "action": "checklister_delete_checklist",
      "resource": "*",
      "effect": "deny"
    }
  ]
}
```

The server has no network listener and no authentication. Anyone who can run the executable can read and modify the SQLite file, so protect the database with normal filesystem permissions and do not expose it to untrusted agents.

Before bulk changes, back up the database:

```sh
sqlite3 /absolute/path/to/checklister.db ".backup '/absolute/path/to/checklister-backup.db'"
```

Using SQLite's `.backup` command is safer than copying the file while the server is running, because it accounts for the write-ahead log.

### Environment variables

| Variable | Used by | Default | Notes |
| --- | --- | --- | --- |
| `DATABASE_URL` | MCP server and CLI | `sqlite://checklister.db` | Absolute paths are recommended: `sqlite:///absolute/path/checklister.db`. |

No other environment variables affect the server.

### Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `unable to open database file` | The parent directory does not exist or is read-only. | `mkdir -p` the parent and make it writable by the agent's user. |
| `database is locked` | Two processes are writing at once. | Retry; keep CLI writes short; do not run long transactions. |
| `checklist not found` or `check not found` | The ID is stale or belongs to another checklist. | Re-list and use current IDs. |
| Agent shows no tools | The host did not load the config or the executable failed. | Run the executable by hand, check `opencode mcp list` or `/mcps`, and inspect the host logs. |
| Server exits immediately | Bad `DATABASE_URL` or unreadable database path. | Run `checklister-mcp` manually and read stderr. |
| Agent calls a tool but the database never changes | The agent process has a different `DATABASE_URL`. | Put `DATABASE_URL` in the server's `environment`/`env` config, not only in your shell. |
| Tools work in the CLI but not in the agent | The CLI and agent point at different files. | Use the same absolute `DATABASE_URL` in both. |
| Docker: `attempt to write a readonly database` | A bind-mounted `/data` is not writable by container uid 1000. | `chown -R 1000:1000` the host directory or use a named volume. |

For OpenCode, server logs are written to `~/.local/share/opencode/log/opencode.log`. Filter for the server name when diagnosing startup failures.

### Running the server by hand

Start the server exactly as the agent would:

```sh
DATABASE_URL=sqlite:///absolute/path/to/checklister.db \
  /absolute/path/to/checklister-mcp
```

It prints nothing and waits for MCP messages on stdin. That is expected. Press `Ctrl+C` to stop it.

To inspect the tools interactively, use any MCP Inspector by configuring it to launch the `checklister-mcp` executable over stdio.

## Docker

Docker is optional. Use it when you want a self-contained Linux runtime or isolation from the host toolchain.

Build the image:

```sh
docker build -t checklister-mcp:local .
```

The image builds the native Rust MCP executable and runs it directly. It does not include Node.js, a JavaScript runtime, or the Rust toolchain at runtime.

Run it with a persistent named volume:

```sh
docker run --rm -i \
  --mount type=volume,source=checklister-data,target=/data \
  checklister-mcp:local
```

The image sets `DATABASE_URL=sqlite:///data/checklister.db`, so the named volume preserves the database across container restarts. The `-i` flag is required for stdio. Do not add `-t`; a TTY can interfere with the MCP message stream.

To use a host directory instead:

```sh
mkdir -p /absolute/path/to/checklister-data
docker run --rm -i \
  --mount type=bind,source=/absolute/path/to/checklister-data,target=/data \
  checklister-mcp:local
```

The container runs as uid 1000. A bind-mounted directory must be writable by that uid.

OpenCode config for the container:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "servers": {
      "checklister": {
        "type": "local",
        "command": [
          "docker", "run", "--rm", "-i",
          "--mount", "type=volume,source=checklister-data,target=/data",
          "checklister-mcp:local"
        ]
      }
    }
  }
}
```

Generic MCP host config:

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

The container uses stdio and does not expose a network port. Each `docker run` starts a fresh process; the named volume is what keeps the data.

## Development

Run formatting, checks, and the full test suite:

```sh
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

The MCP integration tests launch the compiled `checklister-mcp` executable as a real stdio subprocess against temporary SQLite databases. They cover initialization, capabilities, exact tool registration and descriptions, JSON input schemas, annotations, every tool, error handling, invalid requests, persistence across restarts, CLI/MCP database sharing, stdout cleanliness, and clean shutdown.

Useful files:

| Path | Purpose |
| --- | --- |
| `src/bin/checklister-mcp/main.rs` | Native MCP executable entry point, stdio transport, and shutdown. |
| `src/bin/checklister-mcp/server.rs` | MCP server, tool definitions, schemas, handlers, and annotations. |
| `tests/mcp_stdio.rs` | End-to-end stdio integration tests. |
| `src/main.rs` | CLI argument parsing and JSON output. |
| `src/store.rs` | Checklist and check operations shared by the CLI and MCP. |
| `src/storage.rs` | Database connection, busy timeout, and migration lifecycle. |

The CLI binary is built with `cargo build --bin checklister`. The MCP binary is built with `cargo build --bin checklister-mcp`.

## Database and migrations

- `checklists`: auto-incrementing `id` and `name`.
- `checks`: auto-incrementing `id`, parent `checklist_id`, boolean `status` (default `false`), `title`, text `description`, `position`, and `created_at`.

Check order is calculated from creation timestamps, with the check ID as a stable tie-breaker. Deleting a checklist cascades to its checks. Numbered SQL migrations in `migrations/` run when the CLI or MCP server opens the database; older inline-schema databases are adopted and their existing check order is preserved.

The MCP server opens the database once at startup and keeps the existing connection pool for the lifetime of the process. The CLI opens the database per invocation. Both use the same schema, migrations, WAL behavior, foreign-key constraints, and five-second busy timeout.

## Project layout

```text
.
├── src/                          Shared Rust library, CLI, and MCP server
│   ├── lib.rs                    Public library exports
│   ├── main.rs                   CLI argument parsing and JSON output
│   ├── storage.rs                Database connection and migrations
│   ├── store.rs                  Checklist and check operations
│   └── bin/checklister-mcp/      Native Rust MCP server
│       ├── main.rs               stdio entry point
│       └── server.rs             Tools, schemas, and handlers
├── migrations/                   Numbered SQL migrations
├── tests/                        Rust integration tests
│   ├── cli.rs                    CLI persistence and migration tests
│   └── mcp_stdio.rs              End-to-end MCP tests
└── Dockerfile                    Optional native container image
```
