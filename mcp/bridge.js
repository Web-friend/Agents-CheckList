import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const moduleDirectory = dirname(fileURLToPath(import.meta.url));

const libraryNames = {
  linux: "libchecklister_node.so",
  darwin: "libchecklister_node.dylib",
  win32: "checklister_node.dll",
};

function resolveAddon() {
  if (process.env.CHECKLISTER_NATIVE) {
    return process.env.CHECKLISTER_NATIVE;
  }

  const packaged = join(moduleDirectory, "checklister.node");
  if (existsSync(packaged)) {
    return packaged;
  }

  const libraryName = libraryNames[process.platform];
  if (libraryName) {
    for (const profile of ["release", "debug"]) {
      const candidate = join(moduleDirectory, "..", "target", profile, libraryName);
      if (existsSync(candidate)) {
        return candidate;
      }
    }
  }

  throw new Error(
    "Could not find the Checklister native addon. Run `npm --prefix mcp run build:native` or set CHECKLISTER_NATIVE.",
  );
}

const { Database } = require(resolveAddon());

/**
 * Opens the persistent SQLite database through the in-process Rust bridge.
 * Falls back to `DATABASE_URL` (or `sqlite://checklister.db`) when no URL is given.
 */
export function openDatabase(databaseUrl) {
  return Database.open(databaseUrl);
}
