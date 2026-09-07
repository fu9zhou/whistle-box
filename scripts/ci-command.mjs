import { spawnSync } from "node:child_process";

// Keep compiler diagnostics visible in Checks even when log storage is unavailable.
const [command, ...args] = process.argv.slice(2);
if (!command) throw new Error("Missing command");
const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 });
process.stdout.write(result.stdout || "");
process.stderr.write(result.stderr || "");
if (result.error) console.error(result.error.message);
if (result.status !== 0 && process.env.GITHUB_ACTIONS) {
  const tail = `${result.stdout || ""}\n${result.stderr || ""}\n${result.error?.message || ""}`
    .split("\n")
    .slice(-100)
    .join("\n")
    .replaceAll("%", "%25")
    .replaceAll("\r", "%0D")
    .replaceAll("\n", "%0A");
  console.log(`::error::${tail}`);
}
process.exit(result.status ?? 1);
