import { spawn } from "node:child_process";
import { createServer, request } from "node:http";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import assert from "node:assert/strict";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function free() {
  const s = createServer();
  s.listen(0, "127.0.0.1");
  await once(s, "listening");
  const p = s.address().port;
  await new Promise((r) => s.close(r));
  return p;
}
const children = [];
async function fixture(name, port = undefined) {
  port ??= await free();
  const auth = await free();
  const dir = resolve(".tooling/lifecycle", name);
  mkdirSync(dir, { recursive: true });
  const child = spawn(
    resolve("src-tauri/target/debug/examples/ui_fixture.exe"),
    [String(port), String(auth), dir],
    { windowsHide: true, stdio: ["pipe", "pipe", "ignore"] },
  );
  children.push(child);
  const line = once(createInterface({ input: child.stdout }), "line").then(([s]) => JSON.parse(s));
  return {
    child,
    port,
    ready: Promise.race([
      line,
      once(child, "exit").then(() => {
        throw Error("fixture failed");
      }),
      sleep(35000).then(() => {
        throw Error("startup timeout");
      }),
    ]),
  };
}
async function stopped(port) {
  for (let i = 0; i < 40; i++) {
    try {
      await fetch(`http://127.0.0.1:${port}/`, { signal: AbortSignal.timeout(300) });
    } catch {
      return;
    }
    await sleep(100);
  }
  throw Error("owned child remained alive");
}
try {
  const first = await fixture("first");
  const firstInfo = await first.ready;
  const other = await fixture("other");
  const otherInfo = await other.ready;
  const conflict = await fixture("conflict", first.port);
  await assert.rejects(conflict.ready);
  assert.equal(
    (await (await fetch(`http://127.0.0.1:${first.port}/cgi-bin/server-info`)).json()).server.pid,
    firstInfo.pid,
  );
  const exit = once(first.child, "exit");
  first.child.stdin.end();
  await exit;
  await stopped(first.port);
  assert.equal(
    (await (await fetch(`http://127.0.0.1:${other.port}/cgi-bin/server-info`)).json()).server.pid,
    otherInfo.pid,
  );
  console.log(
    "PASS: occupied Whistle port is rejected; stopping owned instance preserves independent Whistle",
  );
  const crashed = once(other.child, "exit");
  other.child.kill();
  await crashed;
  await stopped(other.port);
  console.log("PASS: Windows Job Object reaps embedded child after parent is terminated");
  // Verify the actual launcher forwards via configured upstream proxy.
  const proxy = createServer((req, res) => {
    res.end("upstream:" + req.url);
  });
  proxy.listen(0, "127.0.0.1");
  await once(proxy, "listening");
  try {
    const dir = resolve(".tooling/lifecycle/upstream");
    mkdirSync(dir, { recursive: true });
    const port = await free();
    const child = spawn(
      resolve("src-tauri/binaries/node-x86_64-pc-windows-msvc.exe"),
      [resolve("src-tauri/resources/whistle/launcher.cjs")],
      { windowsHide: true, stdio: ["pipe", "ignore", "ignore"] },
    );
    children.push(child);
    child.stdin.write(
      JSON.stringify({
        host: "127.0.0.1",
        port,
        baseDir: dir,
        upstream_proxy: `http://127.0.0.1:${proxy.address().port}`,
        timeout: 60,
      }) + "\n",
    );
    for (let i = 0; i < 100; i++) {
      try {
        await fetch(`http://127.0.0.1:${port}/cgi-bin/server-info`);
        break;
      } catch {}
      await sleep(100);
    }
    const body = await new Promise((resolve, reject) => {
      const r = request(
        {
          host: "127.0.0.1",
          port,
          path: "http://example.invalid/fixture",
          headers: { Host: "example.invalid" },
          timeout: 5000,
        },
        (res) => {
          let s = "";
          res.on("data", (x) => (s += x));
          res.on("end", () => resolve(s));
        },
      );
      r.on("timeout", () => r.destroy(Error("proxy timeout")));
      r.on("error", reject);
      r.end();
    });
    assert.equal(body, "upstream:http://example.invalid/fixture");
    console.log("PASS: upstream proxy setting affects real HTTP forwarding");
  } finally {
    proxy.closeAllConnections();
    await new Promise((r) => proxy.close(r));
  }
} finally {
  for (const child of children) {
    if (child.exitCode === null && child.signalCode === null) {
      const end = once(child, "exit");
      child.stdin.end();
      setTimeout(() => child.kill(), 3000).unref();
      await end;
    }
  }
}
