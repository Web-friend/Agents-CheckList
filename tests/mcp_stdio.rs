use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

const TOOL_NAMES: [&str; 9] = [
    "create_checklist",
    "list_checklists",
    "show_checklist",
    "delete_checklist",
    "add_check",
    "list_checks",
    "list_undone_checks",
    "complete_check",
    "undo_check",
];

const TOOL_DESCRIPTIONS: [(&str, &str); 9] = [
    (
        "create_checklist",
        "Create a new, empty checklist. Use this first when you need a container for checks; the response contains the generated id required by every checklist-scoped tool. Returns {success, message, result: {id, name}}.",
    ),
    (
        "list_checklists",
        "List every checklist, ordered by id. Use this to discover checklist ids before calling checklist-scoped tools. Returns an array of {id, name}; an empty array means no checklists exist.",
    ),
    (
        "show_checklist",
        "Show one checklist with all of its checks, completed and undone, in creation order. Each check includes id, checklist_id, status, title, description, created_at, and a 1-based order. Returns {checklist, checks}; fails if the checklist does not exist.",
    ),
    (
        "delete_checklist",
        "Permanently delete a checklist and all of its checks. This cannot be undone. Returns the deleted checklist; fails if it does not exist. Prefer confirming with the user before calling when deletion is not clearly intended.",
    ),
    (
        "add_check",
        "Add a new check to an existing checklist. New checks start undone (status false) and are appended in creation order. title must be non-empty; description may be an empty string. Returns the created check; fails if the checklist does not exist.",
    ),
    (
        "list_checks",
        "List all checks in a checklist, complete and undone, in creation order. Returns an array of check objects with id, checklist_id, status, title, description, created_at, and 1-based order, or an empty array. Fails if the checklist does not exist.",
    ),
    (
        "list_undone_checks",
        "List only the still-undone checks (status false) in a checklist, in creation order. Use this to find remaining work or the next check to complete. Fails if the checklist does not exist.",
    ),
    (
        "complete_check",
        "Mark a check complete (status true). The check must belong to checklist_id; completing an already-complete check succeeds without changing it. Returns the updated check; fails if the checklist or check does not exist.",
    ),
    (
        "undo_check",
        "Reopen a completed check by marking it undone (status false). The check must belong to checklist_id; undoing an already-undone check succeeds without changing it. Returns the updated check; fails if the checklist or check does not exist.",
    ),
];

fn database() -> (TempDir, String) {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let url = format!("sqlite://{}", directory.path().join("mcp.sqlite").display());
    (directory, url)
}

struct McpServer {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl McpServer {
    fn start(database_url: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_checklister-mcp"))
            .env("DATABASE_URL", database_url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the MCP server");
        let stdin = child.stdin.take().expect("server stdin");
        let stdout = BufReader::new(child.stdout.take().expect("server stdout"));
        let mut server = Self {
            child,
            stdin: Some(stdin),
            stdout,
            next_id: 1,
        };

        let initialized = server.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "checklister-mcp-test", "version": "0.1.0" }
            }),
        );
        let result = &initialized["result"];
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "checklister");
        assert_eq!(result["serverInfo"]["version"], "0.1.0");
        assert!(result["capabilities"]["tools"].is_object());

        server.notify("notifications/initialized", json!({}));
        server
    }

    fn send_message(&mut self, message: Value) {
        let stdin = self.stdin.as_mut().expect("server stdin is open");
        writeln!(stdin, "{message}").expect("write MCP message");
        stdin.flush().expect("flush MCP message");
    }

    fn read_message(&mut self) -> Value {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).expect("read MCP response");
        assert_ne!(read, 0, "MCP server closed stdout unexpectedly");
        assert!(!line.trim().is_empty(), "MCP server wrote a blank line");
        let message: Value = serde_json::from_str(&line).expect("stdout contains valid JSON");
        assert_eq!(
            message["jsonrpc"], "2.0",
            "stdout contained a non-MCP message: {line}"
        );
        message
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));

        loop {
            let message = self.read_message();
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send_message(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }));
    }

    fn call(&mut self, name: &str, arguments: Value) -> CallOutcome {
        let response = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        assert!(
            response.get("error").is_none(),
            "unexpected protocol error calling {name}: {response}"
        );
        let result = &response["result"];
        let is_error = result["isError"].as_bool().unwrap_or(false);
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("missing text content calling {name}: {response}"))
            .to_owned();
        let payload = serde_json::from_str(&text).unwrap_or(Value::String(text));
        CallOutcome { is_error, payload }
    }

    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let outcome = self.call(name, arguments);
        assert!(
            !outcome.is_error,
            "tool {name} returned an error: {}",
            outcome.payload
        );
        outcome.payload
    }

    fn finish(mut self) -> (ExitStatus, String) {
        drop(self.stdin.take());
        let status = self.child.wait().expect("wait for the MCP server");
        let mut stderr = String::new();
        if let Some(mut error) = self.child.stderr.take() {
            error.read_to_string(&mut stderr).expect("read stderr");
        }
        (status, stderr)
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct CallOutcome {
    is_error: bool,
    payload: Value,
}

fn tool_map(tools: &[Value]) -> BTreeMap<&str, &Value> {
    tools
        .iter()
        .map(|tool| (tool["name"].as_str().expect("tool name"), tool))
        .collect()
}

#[test]
fn server_advertises_exactly_nine_tools_with_schemas_and_annotations() {
    let (_directory, url) = database();
    let mut server = McpServer::start(&url);

    let response = server.request("tools/list", json!({}));
    let tools = response["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), TOOL_NAMES.len());
    let by_name = tool_map(tools);

    for name in TOOL_NAMES {
        let tool = by_name
            .get(name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(tool["name"], name);
        assert!(tool["title"].is_string(), "{name} is missing a title");
        assert!(
            tool["description"]
                .as_str()
                .is_some_and(|text| text.len() > 40),
            "{name} is missing a description"
        );
        assert_eq!(tool["inputSchema"]["type"], "object");
    }

    for (name, description) in TOOL_DESCRIPTIONS {
        assert_eq!(by_name[name]["description"], description);
    }

    let expected_annotations = [
        (
            "create_checklist",
            "Create checklist",
            false,
            false,
            false,
            false,
        ),
        (
            "list_checklists",
            "List checklists",
            true,
            false,
            true,
            false,
        ),
        ("show_checklist", "Show checklist", true, false, true, false),
        (
            "delete_checklist",
            "Delete checklist",
            false,
            true,
            true,
            false,
        ),
        ("add_check", "Add check", false, false, false, false),
        ("list_checks", "List checks", true, false, true, false),
        (
            "list_undone_checks",
            "List undone checks",
            true,
            false,
            true,
            false,
        ),
        (
            "complete_check",
            "Complete check",
            false,
            false,
            true,
            false,
        ),
        ("undo_check", "Undo check", false, false, true, false),
    ];

    for (name, title, read_only, destructive, idempotent, open_world) in expected_annotations {
        let tool = by_name[name];
        assert_eq!(tool["title"], title);
        assert_eq!(tool["annotations"]["readOnlyHint"], read_only);
        assert_eq!(tool["annotations"]["destructiveHint"], destructive);
        assert_eq!(tool["annotations"]["idempotentHint"], idempotent);
        assert_eq!(tool["annotations"]["openWorldHint"], open_world);
    }

    let create = &by_name["create_checklist"]["inputSchema"];
    assert_eq!(create["required"], json!(["name"]));
    assert_eq!(create["properties"]["name"]["type"], "string");
    assert_eq!(create["properties"]["name"]["minLength"], 1);
    assert!(
        create["properties"]["name"]["description"]
            .as_str()
            .unwrap()
            .contains("Must be non-empty")
    );

    let list = &by_name["list_checklists"]["inputSchema"];
    assert_eq!(list["properties"], json!({}));

    let checklist_id = &by_name["show_checklist"]["inputSchema"]["properties"]["checklist_id"];
    assert_eq!(checklist_id["type"], "integer");
    assert_eq!(checklist_id["minimum"], 1);
    assert!(
        checklist_id["description"]
            .as_str()
            .unwrap()
            .contains("create_checklist")
    );

    let add = &by_name["add_check"]["inputSchema"];
    assert_eq!(
        add["required"],
        json!(["checklist_id", "title", "description"])
    );
    assert_eq!(add["properties"]["title"]["minLength"], 1);
    assert!(
        add["properties"]["description"]["description"]
            .as_str()
            .unwrap()
            .contains("empty string")
    );

    let complete = &by_name["complete_check"]["inputSchema"];
    assert_eq!(complete["required"], json!(["checklist_id", "check_id"]));
    assert_eq!(complete["properties"]["check_id"]["minimum"], 1);
    assert!(
        complete["properties"]["check_id"]["description"]
            .as_str()
            .unwrap()
            .contains("within the checklist")
    );

    let (status, stderr) = server.finish();
    assert!(status.success(), "server exited with {status}: {stderr}");
    assert!(stderr.is_empty(), "unexpected stderr output: {stderr}");
}

#[test]
fn all_tools_round_trip_with_existing_response_contracts() {
    let (_directory, url) = database();
    let mut server = McpServer::start(&url);

    let created = server.tool("create_checklist", json!({ "name": "MCP checklist" }));
    assert_eq!(created["success"], true);
    assert_eq!(created["message"], "Checklist created");
    assert_eq!(created["result"]["name"], "MCP checklist");
    let checklist_id = created["result"]["id"].as_i64().expect("checklist id");

    let title = "Vérifier l’API ✓";
    let description = "Description with spaces, unicode ✓ and \"quotes\".";
    let first = server.tool(
        "add_check",
        json!({
            "checklist_id": checklist_id,
            "title": title,
            "description": description,
        }),
    );
    assert_eq!(first["success"], true);
    assert_eq!(first["message"], "Check added");
    assert_eq!(first["result"]["title"], title);
    assert_eq!(first["result"]["description"], description);
    assert_eq!(first["result"]["status"], false);
    assert_eq!(first["result"]["order"], 1);
    assert_eq!(first["result"]["checklist_id"], checklist_id);
    assert!(
        first["result"]["created_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    let first_id = first["result"]["id"].as_i64().expect("check id");

    let second = server.tool(
        "add_check",
        json!({
            "checklist_id": checklist_id,
            "title": "Second",
            "description": "",
        }),
    );
    assert_eq!(second["result"]["description"], "");
    assert_eq!(second["result"]["status"], false);
    assert_eq!(second["result"]["order"], 2);
    let second_id = second["result"]["id"].as_i64().expect("second check id");

    let listed_checklists = server.tool("list_checklists", json!({}));
    assert_eq!(
        listed_checklists,
        json!([{ "id": checklist_id, "name": "MCP checklist" }])
    );

    let checks = server.tool("list_checks", json!({ "checklist_id": checklist_id }));
    assert_eq!(checks.as_array().unwrap().len(), 2);
    assert_eq!(checks[0]["id"], first_id);
    assert_eq!(checks[0]["order"], 1);
    assert_eq!(checks[1]["id"], second_id);
    assert_eq!(checks[1]["order"], 2);

    let shown = server.tool("show_checklist", json!({ "checklist_id": checklist_id }));
    assert_eq!(shown["checklist"]["name"], "MCP checklist");
    assert_eq!(shown["checks"].as_array().unwrap().len(), 2);
    assert_eq!(shown["checks"][0]["id"], first_id);

    let completed = server.tool(
        "complete_check",
        json!({ "checklist_id": checklist_id, "check_id": first_id }),
    );
    assert_eq!(completed["message"], "Check completed");
    assert_eq!(completed["result"]["status"], true);

    let completed_again = server.tool(
        "complete_check",
        json!({ "checklist_id": checklist_id, "check_id": first_id }),
    );
    assert_eq!(completed_again["result"]["status"], true);
    assert_eq!(completed_again["result"]["id"], first_id);

    let undone = server.tool(
        "list_undone_checks",
        json!({ "checklist_id": checklist_id }),
    );
    assert_eq!(undone.as_array().unwrap().len(), 1);
    assert_eq!(undone[0]["id"], second_id);

    let reopened = server.tool(
        "undo_check",
        json!({ "checklist_id": checklist_id, "check_id": first_id }),
    );
    assert_eq!(reopened["message"], "Check marked undone");
    assert_eq!(reopened["result"]["status"], false);

    let reopened_again = server.tool(
        "undo_check",
        json!({ "checklist_id": checklist_id, "check_id": first_id }),
    );
    assert_eq!(reopened_again["result"]["status"], false);

    let other = server.tool("create_checklist", json!({ "name": "Other" }));
    let other_id = other["result"]["id"].as_i64().expect("other checklist id");
    let other_check = server.tool(
        "add_check",
        json!({
            "checklist_id": other_id,
            "title": "Other check",
            "description": "Belongs elsewhere",
        }),
    );
    let other_check_id = other_check["result"]["id"]
        .as_i64()
        .expect("other check id");

    let cross = server.call(
        "complete_check",
        json!({ "checklist_id": checklist_id, "check_id": other_check_id }),
    );
    assert!(cross.is_error);
    assert_eq!(cross.payload["success"], false);
    assert_eq!(cross.payload["error"], "check not found");

    let missing_checklist = server.call("show_checklist", json!({ "checklist_id": 9999 }));
    assert!(missing_checklist.is_error);
    assert_eq!(missing_checklist.payload["error"], "checklist not found");

    let missing_check = server.call(
        "undo_check",
        json!({ "checklist_id": checklist_id, "check_id": 9999 }),
    );
    assert!(missing_check.is_error);
    assert_eq!(missing_check.payload["error"], "check not found");

    let deleted = server.tool("delete_checklist", json!({ "checklist_id": checklist_id }));
    assert_eq!(deleted["message"], "Checklist deleted");
    assert_eq!(deleted["result"]["id"], checklist_id);

    let remaining = server.tool("list_checklists", json!({}));
    assert_eq!(remaining, json!([{ "id": other_id, "name": "Other" }]));

    let (status, stderr) = server.finish();
    assert!(status.success(), "server exited with {status}: {stderr}");
    assert!(stderr.is_empty(), "unexpected stderr output: {stderr}");
}

#[test]
fn invalid_arguments_and_requests_are_handled_without_crashing() {
    let (_directory, url) = database();
    let mut server = McpServer::start(&url);

    let empty_name = server.call("create_checklist", json!({ "name": "" }));
    assert!(empty_name.is_error);
    assert!(empty_name.payload["success"].is_null());

    let zero_id = server.call("show_checklist", json!({ "checklist_id": 0 }));
    assert!(zero_id.is_error);

    let negative_id = server.call("show_checklist", json!({ "checklist_id": -1 }));
    assert!(negative_id.is_error);

    let fractional_id = server.call("show_checklist", json!({ "checklist_id": 1.5 }));
    assert!(fractional_id.is_error);

    let empty_title = {
        let created = server.tool("create_checklist", json!({ "name": "Validate" }));
        let id = created["result"]["id"].as_i64().unwrap();
        server.call(
            "add_check",
            json!({ "checklist_id": id, "title": "", "description": "" }),
        )
    };
    assert!(empty_title.is_error);

    let unknown_tool = server.request(
        "tools/call",
        json!({ "name": "not_a_tool", "arguments": {} }),
    );
    assert_eq!(unknown_tool["error"]["code"], -32602);

    let unknown_method = server.request("not/a/method", json!({}));
    assert_eq!(unknown_method["error"]["code"], -32601);

    server.send_message(json!({ "jsonrpc": "2.0", "id": "bad", "method": 42 }));
    let malformed = server.read_message();
    assert!(malformed.get("error").is_some());

    let alive = server.tool("list_checklists", json!({}));
    assert!(alive.is_array());

    let (status, stderr) = server.finish();
    assert!(status.success(), "server exited with {status}: {stderr}");
}

#[test]
fn database_initialization_failure_goes_to_stderr_only() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let missing = directory.path().join("does-not-exist").join("db.sqlite");
    let output = Command::new(env!("CARGO_BIN_EXE_checklister-mcp"))
        .env("DATABASE_URL", format!("sqlite://{}", missing.display()))
        .stdin(Stdio::null())
        .output()
        .expect("run the MCP server");

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout must stay clean: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unable to open database file") || stderr.contains("checklister-mcp"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn data_persists_across_server_restarts() {
    let (_directory, url) = database();
    let (checklist_id, check_id) = {
        let mut server = McpServer::start(&url);
        let created = server.tool("create_checklist", json!({ "name": "Persistent" }));
        let checklist_id = created["result"]["id"].as_i64().unwrap();
        let added = server.tool(
            "add_check",
            json!({
                "checklist_id": checklist_id,
                "title": "Survives restart",
                "description": "Still here",
            }),
        );
        let check_id = added["result"]["id"].as_i64().unwrap();
        let completed = server.tool(
            "complete_check",
            json!({ "checklist_id": checklist_id, "check_id": check_id }),
        );
        assert_eq!(completed["result"]["status"], true);
        let (status, stderr) = server.finish();
        assert!(status.success(), "{stderr}");
        (checklist_id, check_id)
    };

    let mut restarted = McpServer::start(&url);
    let shown = restarted.tool("show_checklist", json!({ "checklist_id": checklist_id }));
    assert_eq!(shown["checklist"]["name"], "Persistent");
    assert_eq!(shown["checks"].as_array().unwrap().len(), 1);
    assert_eq!(shown["checks"][0]["id"], check_id);
    assert_eq!(shown["checks"][0]["status"], true);
    assert_eq!(shown["checks"][0]["title"], "Survives restart");

    let (status, stderr) = restarted.finish();
    assert!(status.success(), "{stderr}");
}

fn cli_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_checklister"))
}

fn cli(database_url: &str, args: &[&str]) -> Value {
    let binary = cli_binary();
    assert!(
        binary.exists(),
        "the CLI binary is missing at {}; run `cargo test --workspace`",
        binary.display()
    );
    let output = Command::new(binary)
        .args(args)
        .env("DATABASE_URL", database_url)
        .output()
        .expect("run checklister CLI");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI stdout is JSON")
}

#[test]
fn cli_and_mcp_share_the_same_database() {
    let (_directory, url) = database();

    let created = cli(&url, &["checklist", "create", "From CLI"]);
    let checklist_id = created["result"]["id"].as_i64().unwrap();
    let id_arg = checklist_id.to_string();
    let added = cli(
        &url,
        &[
            "check",
            "add",
            &id_arg,
            "--title",
            "CLI check",
            "--description",
            "Visible to MCP",
        ],
    );
    let check_id = added["result"]["id"].as_i64().unwrap();

    let mut server = McpServer::start(&url);
    let visible = server.tool("show_checklist", json!({ "checklist_id": checklist_id }));
    assert_eq!(visible["checklist"]["name"], "From CLI");
    assert_eq!(visible["checks"][0]["id"], check_id);

    let from_mcp = server.tool(
        "add_check",
        json!({
            "checklist_id": checklist_id,
            "title": "MCP check",
            "description": "",
        }),
    );
    let mcp_check_id = from_mcp["result"]["id"].as_i64().unwrap();
    let (status, stderr) = server.finish();
    assert!(status.success(), "{stderr}");

    let shown = cli(&url, &["checklist", "show", &id_arg]);
    assert_eq!(shown["checks"].as_array().unwrap().len(), 2);
    assert_eq!(shown["checks"][1]["id"], mcp_check_id);

    let completed = cli(
        &url,
        &["check", "complete", &id_arg, &mcp_check_id.to_string()],
    );
    assert_eq!(completed["result"]["status"], true);
}
