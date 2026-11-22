import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { Client, InMemoryTransport } from "@modelcontextprotocol/client";
import { createServer } from "./server.js";

test("MCP tools list and call Rust CLI operations", async () => {
  const directory = await mkdtemp(join(tmpdir(), "checklister-mcp-"));
  const databaseUrl = `sqlite://${join(directory, "mcp.sqlite")}`;
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const server = createServer();
  const client = new Client(
    { name: "checklister-mcp-test", version: "0.1.0" },
  );
  const previousDatabaseUrl = process.env.DATABASE_URL;
  const previousBinary = process.env.CHECKLISTER_BIN;
  process.env.DATABASE_URL = databaseUrl;
  process.env.CHECKLISTER_BIN = new URL("../target/debug/checklister", import.meta.url).pathname;

  try {
    await server.connect(serverTransport);
    await client.connect(clientTransport);
    const { tools } = await client.listTools();
    assert.ok(tools.some((tool) => tool.name === "create_checklist"));
    assert.ok(tools.some((tool) => tool.name === "add_check"));

    const created = await client.callTool({
      name: "create_checklist",
      arguments: { name: "MCP checklist" },
    });
    assert.ok(!created.isError);
    const checklist = JSON.parse(created.content[0].text);
    const checklistId = checklist.result.id;

    const added = await client.callTool({
      name: "add_check",
      arguments: {
        checklist_id: checklistId,
        title: "Verify MCP",
        description: "Round-trip through the JavaScript wrapper.",
      },
    });
    assert.ok(!added.isError);
    const check = JSON.parse(added.content[0].text);
    assert.equal(check.result.title, "Verify MCP");
    assert.equal(check.result.description, "Round-trip through the JavaScript wrapper.");
  } finally {
    await client.close();
    await server.close();
    if (previousDatabaseUrl === undefined) delete process.env.DATABASE_URL;
    else process.env.DATABASE_URL = previousDatabaseUrl;
    if (previousBinary === undefined) delete process.env.CHECKLISTER_BIN;
    else process.env.CHECKLISTER_BIN = previousBinary;
    await rm(directory, { recursive: true, force: true });
  }
});
