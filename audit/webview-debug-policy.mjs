import { execFileSync } from 'node:child_process';

// Elevated WebView2 hosts ignore environment flags. Use an app-scoped HKLM
// override only in disposable GitHub runners, and restore it after the test.
export function configureDebugPolicy(browserArguments) {
  if (process.env.GITHUB_ACTIONS !== 'true' || process.env.RUNNER_ENVIRONMENT !== 'github-hosted') return () => {};
  const run = (script) => execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')], { windowsHide: true, encoding: 'utf8', timeout: 20000 }).trim();
  const prefix = String.raw`$ErrorActionPreference='Stop'; $k=[Microsoft.Win32.Registry]::LocalMachine.CreateSubKey('SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments'); $name='whistle-box.exe'; `;
  const encoded = Buffer.from(browserArguments).toString('base64');
  const previous = run(prefix + `$value=$k.GetValue($name,$null); $kind=if($null -ne $value){$k.GetValueKind($name).ToString()}else{'String'}; $k.SetValue($name,[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('${encoded}'))); $k.Close(); @{value=$value;kind=$kind} | ConvertTo-Json -Compress`);
  const snapshot = Buffer.from(previous).toString('base64');
  return () => run(prefix + `$saved=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('${snapshot}')) | ConvertFrom-Json; if($null -eq $saved.value){$k.DeleteValue($name,$false)}else{$k.SetValue($name,$saved.value,[Microsoft.Win32.RegistryValueKind]$saved.kind)}; $k.Close()`);
}
