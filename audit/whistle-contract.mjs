import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createServer } from "node:net";
import assert from "node:assert/strict";
import { once } from "node:events";
const root = resolve(".");
const nativeFetch = globalThis.fetch;
globalThis.fetch = (url, options = {}) =>
  nativeFetch(url, { signal: AbortSignal.timeout(5000), ...options });
const children = [];
async function freePort() {
  const s = createServer();
  s.listen(0, "127.0.0.1");
  await once(s, "listening");
  const p = s.address().port;
  await new Promise((r) => s.close(r));
  return p;
}
async function start(name, rc) {
  const port = await freePort();
  const dir = resolve(".tooling/contract", name);
  mkdirSync(dir, { recursive: true });
  writeFileSync(resolve(dir, ".whistlerc"), "*.username=fixture\n*.password=fixture\n");
  const options = {
    host: "127.0.0.1",
    port,
    baseDir: dir,
    storage: "whistlebox_embedded",
    rcPath: rc ? "none" : undefined,
  };
  const directArgs = [
    "-e",
    "require(process.argv[1])(JSON.parse(process.argv[2]))",
    resolve("src-tauri/resources/whistle/node_modules/whistle"),
    JSON.stringify(options),
  ];
  const child = spawn(process.execPath, directArgs, {
    windowsHide: true,
    cwd: dir,
    env: {
      ...process.env,
      HOME: dir,
      USERPROFILE: dir,
      WHISTLE_PATH: dir,
      WHISTLE_MODE: "",
      HTTP_PROXY: "",
      HTTPS_PROXY: "",
      ALL_PROXY: "",
    },
    stdio: "ignore",
  });
  children.push(child);
  const url = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 100; i++) {
    try {
      const r = await fetch(url + "/cgi-bin/server-info", { signal: AbortSignal.timeout(500) });
      if (r.status === 200 || r.status === 401) return { url, child };
    } catch {}
    if (child.exitCode !== null) throw new Error("fixture exited: " + child.exitCode);
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error("fixture did not start");
}
try {
  const inherited = await start("inherited", false);
  assert.equal((await fetch(inherited.url + "/")).status, 401);
  console.log("CONFIRMED: existing user rc credentials affect embedded launch");
  const isolated = await start("isolated", true);
  const info = await (await fetch(isolated.url + "/cgi-bin/server-info")).json();
  assert.equal(info.server.pid, isolated.child.pid);
  assert.equal(info.server.version, JSON.parse(readFileSync("src-tauri/resources/whistle/package.json", "utf8")).dependencies.whistle);
  console.log("PASS: direct API with rcPath none has owned PID and ignores user rc");
  assert.equal((await fetch(isolated.url + "/cgi-bin/stop")).status, 404);
  console.log("CONFIRMED: /cgi-bin/stop is 404");
  const raw = await fetch(isolated.url + "/cgi-bin/rules/import", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ Default: "example.com 127.0.0.1" }),
  })
    .then((r) => r.json())
    .catch(() => ({ ec: "timeout" }));
  assert.notEqual(raw.ec, 0);
  console.log("CONFIRMED: current raw JSON import rejected, ec=" + raw.ec);
  const form = new FormData();
  form.set(
    "rules",
    new Blob([JSON.stringify({ Default: "example.com 127.0.0.1" })], { type: "application/json" }),
    "rules.json",
  );
  const imported = await (
    await fetch(isolated.url + "/cgi-bin/rules/import", { method: "POST", body: form })
  ).json();
  assert.equal(imported.ec, 0);
  const exported = await (await fetch(isolated.url + "/cgi-bin/rules/export")).json();
  assert.equal(exported.Default, "example.com 127.0.0.1");
  console.log("PASS: multipart import/export round trip");
  const a = await (
    await fetch(inherited.url + "/cgi-bin/rootca", {
      headers: { Authorization: "Basic " + Buffer.from("fixture:fixture").toString("base64") },
    })
  ).text();
  const b = await (await fetch(isolated.url + "/cgi-bin/rootca")).text();
  assert.ok(a.includes("BEGIN CERTIFICATE") && b.includes("BEGIN CERTIFICATE"));
  assert.notEqual(a, b);
  console.log("PASS: independent data roots have different CA certificates");
} finally {
  for (const child of children) {
    if (child.exitCode === null && child.signalCode === null) {
      const closed = once(child, "exit");
      child.kill();
      await closed;
    }
  }
}
