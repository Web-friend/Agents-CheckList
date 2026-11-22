import { McpServer } from "@modelcontextprotocol/server";
import * as z from "zod/v4";

import { runCli } from "./cli.js";

async function callCli(args) {
  try {
    const result = await runCli(args);
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  } catch (error) {
    return {
      isError: true,
      content: [
        {
          type: "text",
          text: JSON.stringify({ success: false, error: error.message }),
        },
      ],
    };
  }
}

export function createServer() {
  const server = new McpServer({ name: "checklister", version: "0.1.0" });

  server.registerTool(
    "create_checklist",
    {
      description: "Create a named checklist and return its generated ID.",
      inputSchema: z.object({ name: z.string().min(1) }),
    },
    ({ name }) => callCli(["checklist", "create", name]),
  );

  server.registerTool(
    "list_checklists",
    { description: "List all checklists.", inputSchema: z.object({}) },
    () => callCli(["checklist", "list"]),
  );

  server.registerTool(
    "show_checklist",
    {
      description: "Show a checklist and all its checks in creation order.",
      inputSchema: z.object({ checklist_id: z.number().int().positive() }),
    },
    ({ checklist_id }) => callCli(["checklist", "show", String(checklist_id)]),
  );

  server.registerTool(
    "delete_checklist",
    {
      description: "Delete a checklist and all its checks.",
      inputSchema: z.object({ checklist_id: z.number().int().positive() }),
    },
    ({ checklist_id }) => callCli(["checklist", "delete", String(checklist_id)]),
  );

  server.registerTool(
    "add_check",
    {
      description: "Add an undone check to a checklist.",
      inputSchema: z.object({
        checklist_id: z.number().int().positive(),
        title: z.string().min(1),
        description: z.string(),
      }),
    },
    ({ checklist_id, title, description }) =>
      callCli([
        "check",
        "add",
        String(checklist_id),
        "--title",
        title,
        "--description",
        description,
      ]),
  );

  server.registerTool(
    "list_checks",
    {
      description: "List a checklist's checks with status and creation order.",
      inputSchema: z.object({ checklist_id: z.number().int().positive() }),
    },
    ({ checklist_id }) => callCli(["check", "list", String(checklist_id)]),
  );

  server.registerTool(
    "list_undone_checks",
    {
      description: "List only the undone checks in a checklist.",
      inputSchema: z.object({ checklist_id: z.number().int().positive() }),
    },
    ({ checklist_id }) => callCli(["check", "undone", String(checklist_id)]),
  );

  server.registerTool(
    "complete_check",
    {
      description: "Mark a check as complete.",
      inputSchema: z.object({
        checklist_id: z.number().int().positive(),
        check_id: z.number().int().positive(),
      }),
    },
    ({ checklist_id, check_id }) =>
      callCli([
        "check",
        "complete",
        String(checklist_id),
        String(check_id),
      ]),
  );

  server.registerTool(
    "undo_check",
    {
      description: "Mark a completed check as undone.",
      inputSchema: z.object({
        checklist_id: z.number().int().positive(),
        check_id: z.number().int().positive(),
      }),
    },
    ({ checklist_id, check_id }) =>
      callCli(["check", "undo", String(checklist_id), String(check_id)]),
  );

  return server;
}
