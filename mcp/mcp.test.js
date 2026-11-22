import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { Client, InMemoryTransport } from "@modelcontextprotocol/client";
import { openDatabase } from "./bridge.js";
import { createServer } from "./server.js";

const toolNames = [
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

async function withServer(run) {
  const directory = await mkdtemp(join(tmpdir(), "checklister-mcp-"));
  const database = await openDatabase(`sqlite://${join(directory, "mcp.sqlite")}`);
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const server = createServer(database);
  const client = new Client({ name: "checklister-mcp-test", version: "0.1.0" });

  try {
    await server.connect(serverTransport);
    await client.connect(clientTransport);
    await run({ client, directory });
  } finally {
    await client.close();
    await server.close();
    await rm(directory, { recursive: true, force: true });
  }
}

function parse(result) {
  return JSON.parse(result.content[0].text);
}

test("MCP tools expose descriptions, parameter docs, and annotations", async () => {
  await withServer(async ({ client }) => {
    const { tools } = await client.listTools();
    const byName = new Map(tools.map((tool) => [tool.name, tool]));

    for (const name of toolNames) {
      const tool = byName.get(name);
      assert.ok(tool, `missing tool ${name}`);
      assert.equal(typeof tool.description, "string", `${name} has no description`);
      assert.ok(tool.description.length > 40, `${name} description is too short`);
      assert.equal(typeof tool.title, "string", `${name} has no title`);
    }

    for (const name of ["list_checklists", "show_checklist", "list_checks", "list_undone_checks"]) {
      assert.equal(byName.get(name).annotations?.readOnlyHint, true, `${name} is not read-only`);
    }
    assert.equal(byName.get("delete_checklist").annotations?.destructiveHint, true);
    assert.equal(byName.get("add_check").annotations?.idempotentHint, false);
    assert.equal(byName.get("complete_check").annotations?.idempotentHint, true);

    const checklistId =
      byName.get("show_checklist").inputSchema.properties.checklist_id;
    assert.equal(typeof checklistId.description, "string");
    assert.match(checklistId.description, /create_checklist/);
  });
});

test("MCP tools round-trip through the native Rust bridge", async () => {
  await withServer(async ({ client }) => {
    const created = await client.callTool({
      name: "create_checklist",
      arguments: { name: "MCP checklist" },
    });
    assert.ok(!created.isError);
    const checklist = parse(created);
    assert.equal(checklist.success, true);
    assert.equal(checklist.message, "Checklist created");
    const checklistId = checklist.result.id;

    const title = "Vérifier l’API ✓";
    const description = 'Description with spaces, unicode ✓ and "quotes".';
    const added = await client.callTool({
      name: "add_check",
      arguments: { checklist_id: checklistId, title, description },
    });
    assert.ok(!added.isError);
    const check = parse(added);
    assert.equal(check.result.title, title);
    assert.equal(check.result.description, description);
    assert.equal(check.result.status, false);
    assert.equal(check.result.order, 1);
    const checkId = check.result.id;

    const completed = await client.callTool({
      name: "complete_check",
      arguments: { checklist_id: checklistId, check_id: checkId },
    });
    assert.ok(!completed.isError);
    assert.equal(parse(completed).result.status, true);

    const undone = await client.callTool({
      name: "list_undone_checks",
      arguments: { checklist_id: checklistId },
    });
    assert.deepEqual(parse(undone), []);

    const shown = await client.callTool({
      name: "show_checklist",
      arguments: { checklist_id: checklistId },
    });
    assert.ok(!shown.isError);
    const details = parse(shown);
    assert.equal(details.checklist.name, "MCP checklist");
    assert.equal(details.checks.length, 1);
    assert.equal(details.checks[0].id, checkId);

    const undoneCheck = await client.callTool({
      name: "undo_check",
      arguments: { checklist_id: checklistId, check_id: checkId },
    });
    assert.ok(!undoneCheck.isError);
    assert.equal(parse(undoneCheck).result.status, false);

    const missing = await client.callTool({
      name: "show_checklist",
      arguments: { checklist_id: 9999 },
    });
    assert.equal(missing.isError, true);
    assert.equal(parse(missing).error, "checklist not found");

    const deleted = await client.callTool({
      name: "delete_checklist",
      arguments: { checklist_id: checklistId },
    });
    assert.ok(!deleted.isError);
    assert.equal(parse(deleted).message, "Checklist deleted");

    const remaining = await client.callTool({ name: "list_checklists", arguments: {} });
    assert.deepEqual(parse(remaining), []);
  });
});
