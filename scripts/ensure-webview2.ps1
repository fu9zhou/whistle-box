$ErrorActionPreference = 'Stop'
function Get-WebViewVersion {
  foreach ($key in @(
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
  )) {
    $version = Get-ItemPropertyValue -LiteralPath $key -Name pv -ErrorAction SilentlyContinue
    if ($version -and [version]$version -gt [version]'0.0.0.0') { return $version }
  }
}
$version = Get-WebViewVersion
if (-not $version) {
  if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw '自动安装 WebView2 仅允许在 GitHub 托管的临时环境运行。'
  }
  $installer = Join-Path $env:RUNNER_TEMP 'MicrosoftEdgeWebview2Setup.exe'
  Invoke-WebRequest 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $installer
  $signature = Get-AuthenticodeSignature -LiteralPath $installer
  if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
    throw 'WebView2 安装程序的微软签名验证失败。'
  }
  $process = Start-Process -FilePath $installer -ArgumentList '/silent /install' -WindowStyle Hidden -PassThru
  if (-not $process.WaitForExit(180000)) { throw 'WebView2 安装超时。' }
  for ($attempt = 0; $attempt -lt 30; $attempt++) {
    $version = Get-WebViewVersion
    if ($version) { break }
    Start-Sleep -Seconds 2
  }
  if (-not $version) { throw "WebView2 安装后仍未检测到运行时，退出码：$($process.ExitCode)" }
}
Write-Output "::notice title=WebView2 运行时::已确认版本 $version"
