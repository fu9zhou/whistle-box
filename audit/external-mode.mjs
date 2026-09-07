import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';

export async function verifyExternalMode(invoke, freePort) {
  const base = await invoke('cmd_get_config');
  const port = await freePort();
  const authPort = await freePort();
  const dir = resolve('.tooling/external-mode');
  mkdirSync(dir, { recursive: true });
  const child = spawn(resolve('src-tauri/target/debug/examples/ui_fixture.exe'), [String(port), String(authPort), dir], {
    windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'],
  });
  child.stderr.resume();
  const lines = createInterface({ input: child.stdout });
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 35000);
  try {
    const [line] = await once(lines, 'line', { signal: controller.signal });
    clearTimeout(timeout);
    const ready = JSON.parse(line);
    const external = structuredClone(base);
    external.whistle.mode = 'external';
    external.app_settings.external_host = '127.0.0.1';
    external.app_settings.external_port = port;
    external.app_settings.external_username = 'fixture';
    external.app_settings.external_password = 'wrong-password';
    await invoke('cmd_save_config', { config: external });
    assert.equal((await invoke('cmd_get_whistle_status')).uptime_check, false);
    external.app_settings.external_password = 'fixture-secret';
    await invoke('cmd_save_config', { config: external });
    await invoke('cmd_start_whistle');
    const status = await invoke('cmd_get_whistle_status');
    assert.equal(status.mode, 'external');
    assert.equal(status.uptime_check, true);
    const url = await invoke('cmd_get_auth_proxy_url');
    const response = await fetch(url, { signal: AbortSignal.timeout(5000) });
    assert.equal(response.status, 200, '更新外部凭据后认证代理应访问成功');
    await invoke('cmd_stop_whistle');
    const info = await (await fetch(`http://127.0.0.1:${port}/cgi-bin/server-info`, { signal: AbortSignal.timeout(5000) })).json();
    assert.equal(info.server.pid, ready.pid, '停止外部模式不能终止外部进程');
    console.log('PASS: actual external mode rejects wrong credentials, applies new credentials and preserves external Whistle');
  } finally {
    clearTimeout(timeout);
    lines.close();
    try {
      await invoke('cmd_save_config', { config: base });
      await invoke('cmd_start_whistle');
    } finally {
      if (child.exitCode === null && child.signalCode === null) {
        const exited = once(child, 'exit');
        child.stdin.end();
        const kill = setTimeout(() => child.kill(), 10000);
        await exited;
        clearTimeout(kill);
      }
    }
  }
}
