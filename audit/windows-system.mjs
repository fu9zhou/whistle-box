import { execFileSync, spawn } from 'node:child_process';
import { X509Certificate } from 'node:crypto';
import { resolve } from 'node:path';
import { once } from 'node:events';
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';

export async function verifyWindowsSystem(nativeInvoke, appPid) {
  if (process.env.GITHUB_ACTIONS !== 'true' || process.env.RUNNER_ENVIRONMENT !== 'github-hosted') {
    throw Error('系统变更验证仅限 GitHub 托管的临时运行环境');
  }
  const invoke = async (command, args) => {
    let timer;
    try {
      return await Promise.race([
        nativeInvoke(command, args),
        new Promise((_, reject) => { timer = setTimeout(() => reject(Error(`Windows 系统命令超时：${command}`)), 30000); }),
      ]);
    } finally { clearTimeout(timer); }
  };
  const ps = (script) => execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')], { windowsHide: true, encoding: 'utf8', timeout: 20000 }).trim();
  const prefix = String.raw`$ErrorActionPreference='Stop'; $k=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Internet Settings',$true); `;
  const read = () => JSON.parse(ps(prefix + "$v=@{}; foreach($n in @('ProxyEnable','ProxyServer','ProxyOverride','AutoConfigURL')) { $v[$n]=$k.GetValue($n,$null) }; $k.Close(); ConvertTo-Json -InputObject $v -Compress"));
  const before = read();
  const autoBefore = await invoke('cmd_get_autostart');
  const certBefore = await invoke('cmd_check_cert_installed');
  assert.equal(certBefore, false, '测试证书必须尚未受信任');
  const config = await invoke('cmd_get_config');
  const certificate = await fetch(`http://127.0.0.1:${config.whistle.port}/cgi-bin/rootca`, {
    headers: { Authorization: `Basic ${Buffer.from(`${config.whistle.username}:${config.whistle.password}`).toString('base64')}` },
    signal: AbortSignal.timeout(5000),
  });
  assert.equal(certificate.status, 200);
  const thumbprint = new X509Certificate(Buffer.from(await certificate.arrayBuffer())).fingerprint.replaceAll(':', '');
  assert.ok(Number.isInteger(appPid) && appPid > 0);
  const confirmCertificate = async (command) => {
    const script = `& {${readFileSync(resolve('audit/confirm-test-certificate.ps1'), 'utf8')}} -AppProcessId ${appPid} -Thumbprint '${thumbprint}'`;
    const helper = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = '';
    for (const stream of [helper.stdout, helper.stderr]) stream.on('data', (data) => { output = (output + data.toString()).slice(-5000); });
    try { await invoke(command); }
    catch (error) { throw Error(`${error.message}\n证书确认诊断：${output}`); }
    finally {
      if (helper.exitCode === null && helper.signalCode === null) {
        const exited = once(helper, 'exit');
        helper.kill();
        await exited;
      }
    }
  };
  try {
    await invoke('cmd_set_proxy_mode', { mode: 'global' });
    assert.equal(read().ProxyEnable, 1);
    assert.equal((await invoke('cmd_get_proxy_status')).mode, 'global');
    await invoke('cmd_set_proxy_mode', { mode: 'rule' });
    const pac = read().AutoConfigURL;
    assert.match(pac, /^http:\/\/127\.0\.0\.1:/);
    assert.match(await (await fetch(pac)).text(), /FindProxyForURL/);
    await invoke('cmd_set_proxy_mode', { mode: 'direct' });
    assert.deepEqual(read(), before, '释放代理必须恢复原值');
    await invoke('cmd_set_proxy_mode', { mode: 'global' });
    ps(prefix + "$k.SetValue('ProxyServer','127.0.0.1:29999'); $k.Close()");
    await invoke('cmd_clear_proxy');
    assert.equal(read().ProxyServer, '127.0.0.1:29999', '保留其他软件的后续修改');
    await confirmCertificate('cmd_install_cert');
    assert.equal(await invoke('cmd_check_cert_installed'), true);
    await invoke('cmd_install_cert');
    await confirmCertificate('cmd_uninstall_cert');
    assert.equal(await invoke('cmd_check_cert_installed'), false);
    await invoke('cmd_set_autostart', { enabled: true });
    assert.equal(await invoke('cmd_get_autostart'), true);
    await invoke('cmd_set_autostart', { enabled: false });
    assert.equal(await invoke('cmd_get_autostart'), false);
    console.log('PASS: Windows 代理接管与恢复、外部修改保留、证书与开机启动');
  } finally {
    await invoke('cmd_clear_proxy').catch(() => {});
    await invoke('cmd_uninstall_cert').catch(() => {});
    await invoke('cmd_set_autostart', { enabled: autoBefore }).catch(() => {});
    const data = Buffer.from(JSON.stringify(before)).toString('base64');
    ps(prefix + `$v=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('${data}')) | ConvertFrom-Json; foreach($p in $v.PSObject.Properties) { if($null -eq $p.Value) { $k.DeleteValue($p.Name,$false) } elseif($p.Name -eq 'ProxyEnable') { $k.SetValue($p.Name,[int]$p.Value,[Microsoft.Win32.RegistryValueKind]::DWord) } else { $k.SetValue($p.Name,[string]$p.Value) } }; $k.Close()`);
  }
}
