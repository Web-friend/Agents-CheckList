import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { runCli } from "./cli.js";

test("CLI bridge returns JSON and preserves descriptions containing spaces", async () => {
  const directory = await mkdtemp(join(tmpdir(), "checklister-mcp-"));
  const previousDatabaseUrl = process.env.DATABASE_URL;
  const previousBinary = process.env.CHECKLISTER_BIN;
  process.env.DATABASE_URL = `sqlite://${join(directory, "test.sqlite")}`;
  process.env.CHECKLISTER_BIN = join(process.cwd(), "../target/debug/checklister");

  try {
    const created = await runCli(["checklist", "create", "MCP bridge"]);
    const checklistId = created.result.id;
    const added = await runCli([
      "check",
      "add",
      String(checklistId),
      "--title",
      "Build release",
      "--description",
      "A long description with spaces.",
    ]);

    assert.equal(added.result.title, "Build release");
    assert.equal(added.result.description, "A long description with spaces.");
    assert.equal(added.result.status, false);
  } finally {
    if (previousDatabaseUrl === undefined) delete process.env.DATABASE_URL;
    else process.env.DATABASE_URL = previousDatabaseUrl;
    if (previousBinary === undefined) delete process.env.CHECKLISTER_BIN;
    else process.env.CHECKLISTER_BIN = previousBinary;
    await rm(directory, { recursive: true, force: true });
  }
});
