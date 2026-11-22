import { execFile as execFileCallback } from "node:child_process";
import { promisify } from "node:util";

const execFile = promisify(execFileCallback);

function errorFromStderr(stderr, fallback) {
  try {
    const parsed = JSON.parse(stderr);
    return parsed.error ?? fallback;
  } catch {
    return stderr.trim() || fallback;
  }
}

export async function runCli(args) {
  const binary = process.env.CHECKLISTER_BIN || "checklister";
  try {
    const { stdout } = await execFile(binary, args, {
      env: process.env,
      maxBuffer: 4 * 1024 * 1024,
    });
    return JSON.parse(stdout);
  } catch (error) {
    if (error.code === "ENOENT") {
      throw new Error(`Could not find Checklister binary: ${binary}`);
    }
    if (error.stdout !== undefined || error.stderr !== undefined) {
      throw new Error(errorFromStderr(error.stderr ?? "", error.message));
    }
    throw error;
  }
}
