param([int]$AppProcessId, [string]$Thumbprint)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
  throw '证书确认自动化仅允许在 GitHub 托管的临时环境运行。'
}
if ($Thumbprint -notmatch '^[A-Fa-f0-9]{40}$') { throw '测试证书指纹无效。' }
if ((Get-Process -Id $AppProcessId).ProcessName -ne 'whistle-box') { throw '测试应用进程不匹配。' }
Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class TestCertDialog {
  public IntPtr Window;
  public IntPtr Yes;
  public string Text;
  delegate bool EnumProc(IntPtr window, IntPtr arg);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback, IntPtr arg);
  [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr window, EnumProc callback, IntPtr arg);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr window, StringBuilder text, int length);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr wParam, StringBuilder text, uint flags, uint timeout, out UIntPtr result);
  [DllImport("user32.dll")] static extern IntPtr GetDlgItem(IntPtr window, int id);
  [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);
  static string Read(IntPtr window) {
    var text = new StringBuilder(8192);
    UIntPtr result;
    if (SendMessageTimeout(window, 0x000D, new UIntPtr((uint)text.Capacity), text, 2, 250, out result) == IntPtr.Zero)
      GetWindowText(window, text, text.Capacity);
    return text.ToString();
  }
  public static TestCertDialog[] Find(uint[] owners) {
    var found = new List<TestCertDialog>();
    EnumWindows((window, arg) => {
      uint pid; GetWindowThreadProcessId(window, out pid);
      if (Array.IndexOf(owners, pid) < 0) return true;
      var text = new StringBuilder(Read(window));
      EnumChildWindows(window, (child, unused) => { text.AppendLine(Read(child)); return true; }, IntPtr.Zero);
      found.Add(new TestCertDialog { Window=window, Yes=GetDlgItem(window, 6), Text=text.ToString() });
      return true;
    }, IntPtr.Zero);
    return found.ToArray();
  }
  public void Confirm() { SendMessage(Yes, 0x00F5, IntPtr.Zero, IntPtr.Zero); }
}
'@
$deadline = [DateTime]::UtcNow.AddSeconds(25)
$lastDialogs = @()
while ([DateTime]::UtcNow -lt $deadline) {
  $owners = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$AppProcessId" | Where-Object Name -eq 'powershell.exe' | ForEach-Object { [uint32]$_.ProcessId })
  if ($owners.Count) {
    $lastDialogs = [TestCertDialog]::Find([uint32[]]$owners)
    foreach ($dialog in $lastDialogs) {
      $normalized = $dialog.Text.ToUpperInvariant() -replace '[^0-9A-F]', ''
      if ($dialog.Yes -ne [IntPtr]::Zero -and $normalized.Contains($Thumbprint.ToUpperInvariant())) {
        $dialog.Confirm()
        Write-Output '已确认仅属于测试进程且指纹完全匹配的证书窗口。'
        exit 0
      }
    }
  }
  Start-Sleep -Milliseconds 300
}
throw "未找到指纹匹配的测试证书窗口。候选内容：$($lastDialogs.Text -join ' | ')"
