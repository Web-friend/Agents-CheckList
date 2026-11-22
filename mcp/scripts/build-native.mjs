import { execFileSync } from "node:child_process";
import { copyFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const moduleDirectory = dirname(fileURLToPath(import.meta.url));
const mcpDirectory = join(moduleDirectory, "..");
const rootDirectory = join(mcpDirectory, "..");

const libraryNames = {
  linux: "libchecklister_node.so",
  darwin: "libchecklister_node.dylib",
  win32: "checklister_node.dll",
};

const libraryName = libraryNames[process.platform];
if (!libraryName) {
  throw new Error(`Unsupported platform for the native addon: ${process.platform}`);
}

const release = process.argv.includes("--release");
const profile = release ? "release" : "debug";
const cargoArgs = [
  "build",
  "-p",
  "checklister-node",
  "--manifest-path",
  join(rootDirectory, "Cargo.toml"),
];
if (release) {
  cargoArgs.push("--release");
}

execFileSync("cargo", cargoArgs, { stdio: "inherit" });

const source = join(rootDirectory, "target", profile, libraryName);
const destination = join(mcpDirectory, "checklister.node");
copyFileSync(source, destination);
console.log(`Copied ${source} -> ${destination}`);
