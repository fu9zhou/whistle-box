import { spawn } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:net";
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { transform } from "esbuild";
import { chromium } from "playwright-core";
import assert from "node:assert/strict";
import { verifyWindowsSystem } from './windows-system.mjs';
const executable = resolve(process.env.WHISTLEBOX_TEST_EXECUTABLE || "src-tauri/target/release/whistle-box.exe");
const defaults = await transform(readFileSync("src/defaults.ts", "utf8"), {
  loader: "ts",
  format: "esm",
});
const { buildFallbackConfig } = await import(
  "data:text/javascript;base64," + Buffer.from(defaults.code).toString("base64")
);
async function free() {
  const s = createServer();
  s.listen(0, "127.0.0.1");
  await once(s, "listening");
  const port = s.address().port;
  await new Promise((r) => s.close(r));
  return port;
}
const dir = resolve(".tooling/webview-fixture");
mkdirSync(dir, { recursive: true });
mkdirSync("test-results", { recursive: true });
const config = buildFallbackConfig();
config.whistle.port = await free();
config.auth_proxy_port = await free();
config.pac_server_port = await free();
config.whistle.username = "fixture";
config.whistle.password = "fixture-secret";
config.whistle.storage_path = resolve(dir, "whistle");
config.app_settings.minimize_to_tray = false;
writeFileSync(resolve(dir, "config.json"), JSON.stringify(config));
const debugPort = await free();
const child = spawn(executable, [], {
  windowsHide: true,
  env: {
    ...process.env,
    RUST_LOG: "info",
    WHISTLEBOX_DATA_DIR: dir,
    WEBVIEW2_USER_DATA_FOLDER: resolve(dir, "webview"),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${debugPort}`,
  },
  stdio: "ignore",
});
let browser;
let page;
const invoke = async (command, args = {}) =>
  page.evaluate(({ command, args }) => window.__TAURI__.core.invoke(command, args), {
    command,
    args,
  });
try {
  for (let i = 0; i < 120; i++) {
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${debugPort}`, { timeout: 800 });
      break;
    } catch {}
    if (child.exitCode !== null) throw Error("native application exited before UI");
    await new Promise((r) => setTimeout(r, 300));
  }
  assert.ok(browser, "WebView2 debugging endpoint not available");
  for (let i = 0; i < 40; i++) {
    page = browser.contexts()[0]?.pages()[0];
    if (page) break;
    await new Promise((r) => setTimeout(r, 250));
  }
  await page.waitForFunction(() => window.__TAURI__?.core?.invoke);
  console.log("PASS: actual Tauri WebView2 initialized");
  await page.getByRole("button", { name: "Whistle", exact: true }).click();
  await page.frameLocator("iframe").locator("#container > *").first().waitFor({ timeout: 40000 });
  await page.screenshot({ path: "test-results/tauri-webview2.png" });
  const before = await invoke("cmd_get_whistle_status");
  assert.ok(before.uptime_check && before.pid > 0);
  await new Promise((r) => setTimeout(r, 18000));
  const after = await invoke("cmd_get_whistle_status");
  assert.equal(after.pid, before.pid);
  console.log("PASS: embedded Whistle rendered inside actual WebView2 and stayed healthy");
  // Real command reconfiguration and rollback, without enabling system proxy.
  let base = await invoke("cmd_get_config");
  let next = structuredClone(base);
  next.auth_proxy_port = await free();
  next.pac_server_port = await free();
  const saved = await invoke("cmd_save_config", { config: next, baseConfig: base });
  assert.equal(saved.auth_proxy_port, next.auth_proxy_port);
  const url = await invoke("cmd_get_auth_proxy_url");
  assert.equal(new URL(url).port, String(next.auth_proxy_port));
  assert.equal(await invoke("cmd_probe_auth_proxy"), true);
  base = await invoke("cmd_get_config");
  next = structuredClone(base);
  next.whistle.port = await free();
  await invoke("cmd_save_config", { config: next, baseConfig: base });
  assert.equal((await invoke("cmd_get_whistle_status")).port, next.whistle.port);
  assert.equal(await invoke("cmd_probe_auth_proxy"), true);
  const blocker = createServer();
  blocker.listen(0, "127.0.0.1");
  await once(blocker, "listening");
  try {
    base = await invoke("cmd_get_config");
    next = structuredClone(base);
    next.auth_proxy_port = blocker.address().port;
    await assert.rejects(invoke("cmd_save_config", { config: next, baseConfig: base }));
    assert.equal((await invoke("cmd_get_config")).auth_proxy_port, base.auth_proxy_port);
    assert.equal(await invoke("cmd_probe_auth_proxy"), true);
  } finally {
    await new Promise((r) => blocker.close(r));
  }
  console.log("PASS: native config applies live port changes and rolls back occupied auth port");
  const cert = await invoke("cmd_check_cert_installed");
  assert.equal(typeof cert, "boolean");
  console.log("PASS: current CA fingerprint check completed read-only");
  if (process.env.WHISTLEBOX_TEST_SYSTEM === '1') await verifyWindowsSystem(invoke);
  await invoke("cmd_stop_whistle");
  assert.equal((await invoke("cmd_get_whistle_status")).running, false);
  const close = once(child, "exit");
  const maintenance = spawn("powershell.exe", ["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File",resolve("src-tauri/resources/maintenance.ps1"),"-Executable",executable], {windowsHide:true,env:{...process.env,WHISTLEBOX_DATA_DIR:dir},stdio:"pipe"});
  const [maintenanceCode]=await once(maintenance,"exit");assert.equal(maintenanceCode,0);await close;
  const cleanup=spawn(executable,["--cleanup-only"],{windowsHide:true,env:{...process.env,WHISTLEBOX_DATA_DIR:dir},stdio:"ignore"});
  const [cleanupCode]=await once(cleanup,"exit");assert.equal(cleanupCode,0);
  console.log("PASS: native owned service stop, installer maintenance shutdown and standalone cleanup entry");

} catch (e) {
  if (page) await page.screenshot({ path: "test-results/tauri-failure.png" }).catch(() => {});
  throw e;
} finally {
  if (child.exitCode === null && child.signalCode === null) {
    try {
      await invoke("cmd_stop_whistle");
    } catch {}
    child.kill();
    await once(child, "exit").catch(() => {});
  }
  await browser?.close().catch(() => {});
}
