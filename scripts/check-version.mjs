import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
const json = (path) => JSON.parse(readFileSync(path, "utf8"));
const version = json("package.json").version;
assert.equal(json("package-lock.json").version, version);
assert.equal(json("src-tauri/tauri.conf.json").version, version);
assert.equal(
  readFileSync("src-tauri/Cargo.toml", "utf8").match(/^version\s*=\s*"([^"]+)"/m)?.[1],
  version,
);
if (process.env.GITHUB_REF_TYPE === "tag") assert.equal(process.env.GITHUB_REF_NAME, `v${version}`);
console.log(`版本一致：${version}`);
