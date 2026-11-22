import { McpServer } from "@modelcontextprotocol/server";
import * as z from "zod/v4";

const checklistIdSchema = z
  .number()
  .int()
  .positive()
  .describe(
    "ID of the checklist. Use create_checklist or list_checklists to obtain it.",
  );

const checkIdSchema = z
  .number()
  .int()
  .positive()
  .describe(
    "ID of the check within the checklist. Use add_check, list_checks, or show_checklist to obtain it.",
  );

function textResponse(payload) {
  return {
    content: [{ type: "text", text: JSON.stringify(payload, null, 2) }],
  };
}

async function run(action) {
  try {
    return textResponse(await action());
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

function mutation(message, action) {
  return run(async () => ({ success: true, message, result: await action() }));
}

export function createServer(database) {
  const server = new McpServer({ name: "checklister", version: "0.1.0" });

  server.registerTool(
    "create_checklist",
    {
      title: "Create checklist",
      description:
        "Create a new, empty checklist. Use this first when you need a container for checks; the response contains the generated id required by every checklist-scoped tool. Returns {success, message, result: {id, name}}.",
      inputSchema: z.object({
        name: z
          .string()
          .min(1)
          .describe("Display name for the new checklist. Must be non-empty."),
      }),
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: false,
      },
    },
    ({ name }) => mutation("Checklist created", () => database.createChecklist(name)),
  );

  server.registerTool(
    "list_checklists",
    {
      title: "List checklists",
      description:
        "List every checklist, ordered by id. Use this to discover checklist ids before calling checklist-scoped tools. Returns an array of {id, name}; an empty array means no checklists exist.",
      inputSchema: z.object({}),
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    () => run(() => database.listChecklists()),
  );

  server.registerTool(
    "show_checklist",
    {
      title: "Show checklist",
      description:
        "Show one checklist with all of its checks, completed and undone, in creation order. Each check includes id, checklist_id, status, title, description, created_at, and a 1-based order. Returns {checklist, checks}; fails if the checklist does not exist.",
      inputSchema: z.object({ checklist_id: checklistIdSchema }),
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    ({ checklist_id }) => run(() => database.showChecklist(checklist_id)),
  );

  server.registerTool(
    "delete_checklist",
    {
      title: "Delete checklist",
      description:
        "Permanently delete a checklist and all of its checks. This cannot be undone. Returns the deleted checklist; fails if it does not exist. Prefer confirming with the user before calling when deletion is not clearly intended.",
      inputSchema: z.object({ checklist_id: checklistIdSchema }),
      annotations: {
        readOnlyHint: false,
        destructiveHint: true,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    ({ checklist_id }) =>
      mutation("Checklist deleted", () => database.deleteChecklist(checklist_id)),
  );

  server.registerTool(
    "add_check",
    {
      title: "Add check",
      description:
        "Add a new check to an existing checklist. New checks start undone (status false) and are appended in creation order. title must be non-empty; description may be an empty string. Returns the created check; fails if the checklist does not exist.",
      inputSchema: z.object({
        checklist_id: checklistIdSchema,
        title: z
          .string()
          .min(1)
          .describe("Short title for the new check. Must be non-empty."),
        description: z
          .string()
          .describe(
            "Detailed description for the new check. Use an empty string when there is none.",
          ),
      }),
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: false,
      },
    },
    ({ checklist_id, title, description }) =>
      mutation("Check added", () =>
        database.addCheck(checklist_id, title, description),
      ),
  );

  server.registerTool(
    "list_checks",
    {
      title: "List checks",
      description:
        "List all checks in a checklist, complete and undone, in creation order. Returns an array of check objects with id, checklist_id, status, title, description, created_at, and 1-based order, or an empty array. Fails if the checklist does not exist.",
      inputSchema: z.object({ checklist_id: checklistIdSchema }),
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    ({ checklist_id }) => run(() => database.listChecks(checklist_id, false)),
  );

  server.registerTool(
    "list_undone_checks",
    {
      title: "List undone checks",
      description:
        "List only the still-undone checks (status false) in a checklist, in creation order. Use this to find remaining work or the next check to complete. Fails if the checklist does not exist.",
      inputSchema: z.object({ checklist_id: checklistIdSchema }),
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    ({ checklist_id }) => run(() => database.listChecks(checklist_id, true)),
  );

  server.registerTool(
    "complete_check",
    {
      title: "Complete check",
      description:
        "Mark a check complete (status true). The check must belong to checklist_id; completing an already-complete check succeeds without changing it. Returns the updated check; fails if the checklist or check does not exist.",
      inputSchema: z.object({
        checklist_id: checklistIdSchema,
        check_id: checkIdSchema,
      }),
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    ({ checklist_id, check_id }) =>
      mutation("Check completed", () =>
        database.completeCheck(checklist_id, check_id),
      ),
  );

  server.registerTool(
    "undo_check",
    {
      title: "Undo check",
      description:
        "Reopen a completed check by marking it undone (status false). The check must belong to checklist_id; undoing an already-undone check succeeds without changing it. Returns the updated check; fails if the checklist or check does not exist.",
      inputSchema: z.object({
        checklist_id: checklistIdSchema,
        check_id: checkIdSchema,
      }),
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    ({ checklist_id, check_id }) =>
      mutation("Check marked undone", () =>
        database.undoCheck(checklist_id, check_id),
      ),
  );

  return server;
}
