import { serveStdio } from "@modelcontextprotocol/server/stdio";

import { createServer } from "./server.js";

const stdio = serveStdio(createServer, {
  onerror: (error) => console.error("MCP server error:", error),
});

process.once("SIGINT", () => void stdio.close());
process.once("SIGTERM", () => void stdio.close());
