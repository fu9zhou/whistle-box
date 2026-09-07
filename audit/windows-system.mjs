import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';

export async function verifyWindowsSystem(invoke) {
  if (process.env.GITHUB_ACTIONS !== 'true' || process.env.RUNNER_ENVIRONMENT !== 'github-hosted') {
    throw Error('系统变更验证仅限 GitHub 托管的临时运行环境');
  }
  const ps = (script) => execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')], { windowsHide: true, encoding: 'utf8' }).trim();
  const prefix = "$ErrorActionPreference='Stop'; $k=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Internet Settings',$true); ";
  const read = () => JSON.parse(ps(prefix + "$v=@{}; foreach($n in @('ProxyEnable','ProxyServer','ProxyOverride','AutoConfigURL')) { $v[$n]=$k.GetValue($n,$null) }; $k.Close(); ConvertTo-Json -InputObject $v -Compress"));
  const before = read();
  const autoBefore = await invoke('cmd_get_autostart');
  const certBefore = await invoke('cmd_check_cert_installed');
  assert.equal(certBefore, false, '测试证书必须尚未受信任');
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
    await invoke('cmd_install_cert');
    assert.equal(await invoke('cmd_check_cert_installed'), true);
    await invoke('cmd_install_cert');
    await invoke('cmd_uninstall_cert');
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
