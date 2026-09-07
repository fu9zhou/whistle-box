import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { once } from "node:events";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { createInterface } from "node:readline";
import { chromium } from "playwright-core";
import assert from "node:assert/strict";
async function port() {
  const s = createServer();
  s.listen(0, "127.0.0.1");
  await once(s, "listening");
  const p = s.address().port;
  await new Promise((r) => s.close(r));
  return p;
}
const dir = resolve(".tooling/browser-fixture");
mkdirSync(dir, { recursive: true });
mkdirSync("test-results", { recursive: true });
writeFileSync(resolve(dir, ".whistlerc"), "*.username=wrong\n*.password=wrong\n");
const whistlePort = await port(),
  authPort = await port();
const child = spawn(
  resolve("src-tauri/target/debug/examples/ui_fixture.exe"),
  [String(whistlePort), String(authPort), dir],
  {
    windowsHide: true,
    env: {
      ...process.env,
      HOME: dir,
      USERPROFILE: dir,
      WHISTLE_MODE: "network|notAllowed",
      WHISTLE_PATH: dir,
    },
    stdio: ["pipe", "pipe", "pipe"],
  },
);
let browser, host;
let stderr = "";
child.stderr.on("data", (x) => (stderr += x));
try {
  const ready = await Promise.race([
    once(createInterface({ input: child.stdout }), "line").then(([s]) => JSON.parse(s)),
    once(child, "exit").then(() => {
      throw Error("fixture exited: " + stderr);
    }),
    new Promise((_, reject) =>
      setTimeout(() => reject(Error("fixture readiness timeout: " + stderr)), 40000).unref(),
    ),
  ]);
  const info = await (await fetch(`http://127.0.0.1:${whistlePort}/cgi-bin/server-info`)).json();
  assert.equal(info.server.pid, ready.pid);
  assert.equal((await fetch(`http://127.0.0.1:${whistlePort}/`)).status, 401);
  assert.equal((await fetch(`http://127.0.0.1:${authPort}/?_token=bad`)).status, 403);
  host = createServer((req, res) => {
    res.setHeader("Content-Type", "text/html");
    res.end(
      `<html><body style="margin:0"><iframe src="${ready.url}" style="width:100vw;height:100vh;border:0"></iframe><script>window.messages=[];addEventListener('message',e=>messages.push(e.data.type))</script></body></html>`,
    );
  });
  host.listen(0, "127.0.0.1");
  await once(host, "listening");
  browser = await chromium.launch({
    executablePath: "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    headless: true,
  });
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await context.newPage();
  const failures = [],
    errors = [];
  const cdp = await context.newCDPSession(page);
  await cdp.send("Network.enable");
  cdp.on("Network.responseReceivedExtraInfo", (e) => {
    if (e.blockedCookies?.length)
      console.log(
        "Blocked cookie reasons:",
        e.blockedCookies.map((x) => x.blockedReasons),
      );
  });
  page.on("response", (r) => {
    if (r.status() >= 400) failures.push({ path: new URL(r.url()).pathname, status: r.status });
  });
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`http://localhost:${host.address().port}`);
  const frame = page.frameLocator("iframe");
  await frame.locator("#container > *").first().waitFor({ timeout: 30000 });
  await page.waitForFunction(() => window.messages.includes("whistlebox-ui-ready"));
  await page.screenshot({ path: "test-results/whistle-embedded.png" });
  assert.ok(await frame.locator("#container").innerText(), "Whistle rendered no text");
  const cookies = await context.cookies();
  console.log(
    "Cookie metadata:",
    cookies.map((c) => ({ name: c.name, secure: c.secure, sameSite: c.sameSite })),
  );
  assert.ok(cookies.some((c) => c.name === `whistlebox_session_${authPort}`));
  const nested = await page.frames()[1].evaluate(async () => {
    const r = await fetch("/cgi-bin/server-info", { referrer: location.origin + "/js/" });
    return { status: r.status, info: await r.json() };
  });
  assert.equal(nested.status, 200);
  assert.equal(nested.info.server.pid, ready.pid);
  await page.screenshot({ path: "test-results/whistle-embedded.png" });
  assert.deepEqual(failures, []);
  assert.deepEqual(errors, []);
  console.log(
    "PASS: actual launcher ignores global rc/env, owns server PID, authenticated iframe renders, session persists and JS/resources have no errors",
  );
  child.stdin.end();
  await once(child, "exit");
  await assert.rejects(
    fetch(`http://127.0.0.1:${whistlePort}/`, { signal: AbortSignal.timeout(1000) }),
  );
  console.log("PASS: stdin close stops owned Whistle process");
} finally {
  await browser?.close();
  if (host) await new Promise((r) => host.close(r));
  if (child.exitCode === null && child.signalCode === null) {
    child.stdin.end();
    const close = once(child, "exit");
    setTimeout(() => child.kill(), 3000).unref();
    await close;
  }
}
