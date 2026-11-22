import { serveStdio } from "@modelcontextprotocol/server/stdio";

import { openDatabase } from "./bridge.js";
import { createServer } from "./server.js";

const database = await openDatabase();

const stdio = serveStdio(() => createServer(database), {
  onerror: (error) => console.error("MCP server error:", error),
});

process.once("SIGINT", () => void stdio.close());
process.once("SIGTERM", () => void stdio.close());
