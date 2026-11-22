# Checklister

Checklister is a Rust checklist CLI backed by a persistent SQLite database. A JavaScript MCP server exposes the same operations to AI agents through an in-process Node-API (FFI) addon. The Rust library remains the source of truth for checklist behavior.

- **CLI** — `checklister` for manual use and scripts.
- **MCP server** — `mcp/` for AI agents such as OpenCode, Claude Desktop, Codex, Cursor, and Zed.
- **Native addon** — `crates/checklister-node/`, built into `mcp/checklister.node` and loaded directly by Node.

## Contents

- [Quick start](#quick-start)
- [CLI usage](#cli-usage)
- [Using the MCP server with an AI agent](#using-the-mcp-server-with-an-ai-agent)
  - [How it works](#how-it-works)
  - [Requirements](#requirements)
  - [Build the native addon](#build-the-native-addon)
  - [Choose a database](#choose-a-database)
  - [Connect an agent](#connect-an-agent)
  - [Verify the connection](#verify-the-connection)
  - [What the agent can do](#what-the-agent-can-do)
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

Expose the same store to an AI agent:

```sh
npm --prefix mcp ci
npm --prefix mcp run build:native
```

Then point your agent at this command with an absolute database path:

```sh
DATABASE_URL=sqlite:///absolute/path/to/checklister.db \
  node /absolute/path/to/checklister/mcp/index.js
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
┌─────────────┐   MCP over stdio   ┌──────────────────┐   Node-API FFI   ┌──────────────────┐   SQL   ┌────────────┐
│  AI agent   │ ◄────────────────► │  mcp/index.js    │ ◄──────────────► │ checklister.node │ ◄─────► │  SQLite    │
│  (MCP host) │                    │  (Node process)  │                  │  (Rust addon)    │         │  database  │
└─────────────┘                    └──────────────────┘                  └──────────────────┘         └────────────┘
```

- The agent starts `node mcp/index.js` as a child process and speaks MCP over stdin/stdout.
- `mcp/server.js` registers nine tools and validates their arguments with Zod.
- `mcp/bridge.js` loads the native addon and opens the SQLite database once at startup.
- The addon calls the Rust `Store` directly. No CLI subprocess is spawned per tool call.
- Rust errors such as `checklist not found` are returned to the agent as tool errors.
- Diagnostics go to stderr. stdout is reserved for MCP protocol messages.

Because the database is opened once, migrations run at server startup rather than on every call. The CLI and the MCP server can use the same database file, but avoid writing from both at the same time for long periods; SQLite serializes writers and Checklister uses a five-second busy timeout.

### Requirements

| Requirement | Needed for | Notes |
| --- | --- | --- |
| Node.js 20 or newer | Running the MCP server | Node-API addons and ESM top-level await are used. |
| Rust 1.88 or newer | Building the addon | Not needed at runtime. napi-rs 3 requires 1.88+. |
| A writable database path | Running the server | The parent directory must exist. The file is created if missing. |
| An MCP-capable agent | Using the tools | OpenCode, Claude Desktop, Codex, Cursor, Zed, or any stdio MCP host. |

The addon is platform-specific. Build it on the operating system, architecture, and libc family where the agent will run. Rebuilding is a single command and does not require changing any JavaScript.

### Build the native addon

Install JavaScript dependencies and compile the Rust addon:

```sh
npm --prefix mcp ci
npm --prefix mcp run build:native
```

The build script runs `cargo build --release -p checklister-node` and copies the shared library to `mcp/checklister.node`. Verify it exists:

```sh
ls -l mcp/checklister.node
```

Useful variants:

```sh
# Faster, unoptimized build for development
npm --prefix mcp run build:native:debug

# Production install without test-only dependencies
npm --prefix mcp ci --omit=dev
```

The loader resolves the addon in this order:

1. The path in `CHECKLISTER_NATIVE`, when set.
2. `mcp/checklister.node` (the packaged build).
3. `target/release/libchecklister_node.so` (or the platform equivalent).
4. `target/debug/libchecklister_node.so` (or the platform equivalent).

If none of these exist, the server fails at startup with an instruction to run `build:native`. Set `CHECKLISTER_NATIVE` when the addon lives outside the `mcp/` directory, for example in a custom deployment bundle.

The addon is loaded into the Node process. Keep `mcp/index.js`, `mcp/server.js`, `mcp/bridge.js`, `mcp/package.json`, `mcp/node_modules`, and `mcp/checklister.node` together. The `mcp/scripts/` directory is only needed to rebuild.

### Choose a database

`DATABASE_URL` controls which SQLite file is used. Absolute paths are strongly recommended, because the working directory differs between the CLI, `npm --prefix mcp start`, and an agent-launched process.

```sh
# Absolute path (three slashes after sqlite:)
DATABASE_URL=sqlite:///absolute/path/to/checklister.db

# Relative path (resolved from the process working directory)
DATABASE_URL=sqlite://checklister.db
```

When `DATABASE_URL` is unset, the server uses `sqlite://checklister.db` relative to its working directory. With `npm --prefix mcp start`, that is `mcp/checklister.db`. With an agent that launches the script from a workspace root, it is the workspace root. Prefer an absolute path to avoid surprises.

Create the parent directory before starting the server:

```sh
mkdir -p /absolute/path/to/data
```

SQLite creates the database file and the `-wal` and `-shm` files on first use. Migrations run automatically. Deleting the database file resets all checklists.

### Connect an agent

Every MCP host needs the same two things: the command that starts the server and the `DATABASE_URL` environment variable. Use absolute paths everywhere.

#### OpenCode (V2)

OpenCode V2 configures MCP servers under `mcp.servers`. Add this to `opencode.jsonc` in the project, or to the global config at `~/.config/opencode/opencode.jsonc`:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "servers": {
      "checklister": {
        "type": "local",
        "command": ["node", "/absolute/path/to/checklister/mcp/index.js"],
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
  -- node /absolute/path/to/checklister/mcp/index.js

opencode mcp list
```

OpenCode reloads configuration after the file is saved; no restart is required. Use `/mcps` to inspect the connection, connect, or disconnect the server.

OpenCode enables Code Mode for local MCP servers by default. Tools are grouped under the normalized server name, so the agent calls `tools.checklister.create_checklist(...)` and sees tool names such as `checklister_create_checklist`. Set `"codemode": false` on the server entry if you want the tools on the provider's native tool list instead.

To keep the entry configured without connecting it:

```jsonc
{
  "type": "local",
  "command": ["node", "/absolute/path/to/checklister/mcp/index.js"],
  "disabled": true
}
```

#### Claude Desktop

Edit the Claude Desktop configuration file:

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "checklister": {
      "command": "/absolute/path/to/node",
      "args": ["/absolute/path/to/checklister/mcp/index.js"],
      "env": {
        "DATABASE_URL": "sqlite:///absolute/path/to/checklister.db"
      }
    }
  }
}
```

Restart Claude Desktop after editing the file. Use the absolute path to `node` (find it with `which node` or `where node`) because desktop applications often do not inherit the shell `PATH` that knows about `nvm`, `fnm`, or Homebrew.

#### Other MCP hosts

Most hosts accept the same `mcpServers` shape:

```json
{
  "mcpServers": {
    "checklister": {
      "command": "node",
      "args": ["/absolute/path/to/checklister/mcp/index.js"],
      "env": {
        "DATABASE_URL": "sqlite:///absolute/path/to/checklister.db"
      }
    }
  }
}
```

Some hosts use a single `command` array instead of separate `command` and `args`, and some call the environment map `environment` or `env`. The values are the same:

```text
node /absolute/path/to/checklister/mcp/index.js
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

### What the agent can do

The agent has nine tools. It can chain them to plan work, report progress, and clean up:

| Goal | Tools the agent uses |
| --- | --- |
| Create a checklist | `create_checklist` |
| Discover checklist IDs | `list_checklists` |
| Inspect everything in a checklist | `show_checklist` |
| Add work items | `add_check` |
| See all checks, including completed ones | `list_checks` |
| See only remaining work | `list_undone_checks` |
| Mark work done | `complete_check` |
| Reopen work | `undo_check` |
| Remove a checklist and all its checks | `delete_checklist` |

Example prompts that work well:

```text
Create a checklist named "Release 1.0" and add checks for running tests,
updating the changelog, and tagging the release.
```

```text
What is still undone on checklist 1? Summarize the remaining work.
```

```text
Mark the "Run tests" check on checklist 1 as complete.
```

```text
Reopen the changelog check on checklist 1; we still need to update it.
```

```text
Show me checklist 2 and tell me how many checks are complete.
```

```text
Delete checklist 3. Ask me to confirm before you do it.
```

Because every response is JSON, the agent can also build its own summaries, compare checklists, or report progress without additional tooling.

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
  "description": "Run cargo test and npm --prefix mcp test.",
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
  "description": "Run cargo test and npm --prefix mcp test."
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
      "description": "Run cargo test and npm --prefix mcp test.",
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

Invalid arguments, such as a non-positive ID or an empty checklist name, are rejected by the tool schema before the Rust store is called.

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

The server has no network listener and no authentication. Anyone who can run the Node process can read and modify the SQLite file, so protect the database with normal filesystem permissions and do not expose it to untrusted agents.

Before bulk changes, back up the database:

```sh
sqlite3 /absolute/path/to/checklister.db ".backup '/absolute/path/to/checklister-backup.db'"
```

Using SQLite's `.backup` command is safer than copying the file while the server is running, because it accounts for the write-ahead log.

### Environment variables

| Variable | Used by | Default | Notes |
| --- | --- | --- | --- |
| `DATABASE_URL` | MCP server and CLI | `sqlite://checklister.db` | Absolute paths are recommended: `sqlite:///absolute/path/checklister.db`. |
| `CHECKLISTER_NATIVE` | MCP bridge | unset | Absolute path to a custom `checklister.node` addon. |
| `NODE_ENV` | Docker image | `production` | Does not change server behavior. |

`CHECKLISTER_BIN` is no longer used. The MCP server no longer invokes the CLI binary; it calls the Rust library in-process. The CLI itself is still available for manual use.

### Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `Could not find the Checklister native addon` | The addon was not built or was moved. | Run `npm --prefix mcp run build:native` or set `CHECKLISTER_NATIVE`. |
| `Cannot find package '@modelcontextprotocol/server'` | `mcp/node_modules` is missing. | Run `npm --prefix mcp ci`. |
| `invalid ELF header`, `wrong ELF class`, or `cannot open shared object file` | The addon was built for a different OS, architecture, or libc. | Rebuild on the target machine with `npm --prefix mcp run build:native`. |
| `unable to open database file` | The parent directory does not exist or is read-only. | `mkdir -p` the parent and make it writable by the agent's user. |
| `database is locked` | Two processes are writing at once. | Retry; keep CLI writes short; do not run long transactions. |
| `checklist not found` or `check not found` | The ID is stale or belongs to another checklist. | Re-list and use current IDs. |
| Agent shows no tools | The host did not load the config or the command failed. | Run the command by hand, check `opencode mcp list` or `/mcps`, and inspect the host logs. |
| Server exits immediately | Missing Node version, addon, dependency, or bad `DATABASE_URL`. | Run `node /absolute/path/to/mcp/index.js` manually and read stderr. |
| Agent calls a tool but the database never changes | The agent process has a different `DATABASE_URL`. | Put `DATABASE_URL` in the server's `environment`/`env` config, not only in your shell. |
| Tools work in the CLI but not in the agent | The CLI and agent point at different files. | Use the same absolute `DATABASE_URL` in both. |
| Docker: `attempt to write a readonly database` | A bind-mounted `/data` is not writable by container uid 1000. | `chown -R 1000:1000` the host directory or use a named volume. |

For OpenCode, server logs are written to `~/.local/share/opencode/log/opencode.log`. Filter for the server name when diagnosing startup failures.

### Running the server by hand

Start the server exactly as the agent would:

```sh
DATABASE_URL=sqlite:///absolute/path/to/checklister.db \
  node /absolute/path/to/checklister/mcp/index.js
```

It prints nothing and waits for MCP messages on stdin. That is expected. Press `Ctrl+C` to stop it.

To inspect the tools interactively, use the official MCP Inspector:

```sh
DATABASE_URL=sqlite:///absolute/path/to/checklister.db \
  npx @modelcontextprotocol/inspector node /absolute/path/to/checklister/mcp/index.js
```

The inspector opens a browser UI where you can list tools, fill in arguments, and call them. This is the fastest way to verify the server without involving an agent.

## Docker

Docker is optional. Use it when you want a self-contained Linux runtime, a prebuilt addon, or isolation from the host toolchain. Local mode is simpler and does not need Docker.

Build the image:

```sh
docker build -t checklister-mcp:local .
```

The image builds the Rust addon during the image build and ships `checklister.node`; it does not include the Rust toolchain or the CLI binary at runtime.

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

The container runs as the `node` user (uid 1000). A bind-mounted directory must be writable by that uid.

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

Run the Rust tests:

```sh
cargo test
```

Run the MCP tests. This builds the debug addon first:

```sh
npm --prefix mcp test
```

The MCP tests cover the nine tool registrations, descriptions, parameter docs, annotations, a full create → add → complete → show → undo → delete round trip, missing-ID errors, and descriptions containing spaces and unicode.

Build the addon in debug mode for faster iteration:

```sh
npm --prefix mcp run build:native:debug
```

Useful files:

| Path | Purpose |
| --- | --- |
| `mcp/index.js` | Entry point; opens the database and serves stdio. |
| `mcp/server.js` | Tool definitions, schemas, descriptions, and annotations. |
| `mcp/bridge.js` | Loads the native addon and exposes `openDatabase`. |
| `mcp/scripts/build-native.mjs` | Builds the addon and copies it to `mcp/checklister.node`. |
| `crates/checklister-node/src/lib.rs` | Rust Node-API bindings for the `Database` class. |
| `src/store.rs` | Checklist and check operations shared by the CLI and MCP. |

The CLI binary is still built by `cargo build --bin checklister`. The addon is built separately by `cargo build -p checklister-node` or the npm script.

## Database and migrations

- `checklists`: auto-incrementing `id` and `name`.
- `checks`: auto-incrementing `id`, parent `checklist_id`, boolean `status` (default `false`), `title`, text `description`, and `created_at`.

Check order is calculated from creation timestamps, with the check ID as a stable tie-breaker. Deleting a checklist cascades to its checks. Numbered SQL migrations in `migrations/` run when the CLI or MCP server first opens the database; older inline-schema databases are adopted and their existing check order is preserved.

The MCP server opens the database once at startup and keeps a connection pool for the lifetime of the process. The CLI opens the database per invocation. Both use the same schema and migrations.

## Project layout

```text
.
├── src/                         Rust library and CLI
│   ├── lib.rs                   Public library exports
│   ├── main.rs                  CLI argument parsing and JSON output
│   ├── storage.rs               Database connection and migrations
│   └── store.rs                 Checklist and check operations
├── crates/checklister-node/     Node-API addon
│   ├── Cargo.toml
│   ├── build.rs
│   └── src/lib.rs
├── mcp/                         MCP server
│   ├── index.js                 stdio entry point
│   ├── server.js                Tool registrations
│   ├── bridge.js                Native addon loader
│   ├── mcp.test.js              MCP integration tests
│   └── scripts/build-native.mjs Addon build helper
├── migrations/                  Numbered SQL migrations
├── tests/                       Rust integration tests
└── Dockerfile                   Optional container image
```
