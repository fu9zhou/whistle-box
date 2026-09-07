$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
  throw '安装验证仅允许在 GitHub 托管的临时 Windows 环境运行。'
}
$testInstall = Join-Path $env:RUNNER_TEMP 'WhistleBox-install-test'
$installer = Get-ChildItem (Join-Path $PSScriptRoot '../release/*-setup.exe') | Select-Object -First 1
if (-not $installer) { throw '未找到安装包' }
foreach ($attempt in 1..2) {
  $installProcess = Start-Process -FilePath $installer.FullName -ArgumentList @('/S', "/D=$testInstall") -PassThru -WindowStyle Hidden
  if (-not $installProcess.WaitForExit(120000)) { throw '安装或覆盖升级超时' }
  if ($installProcess.ExitCode -ne 0) { throw "安装失败：$($installProcess.ExitCode)" }
  foreach ($file in @('whistle-box.exe','node.exe','resources/whistle/launcher.cjs','resources/maintenance.ps1')) {
    if (-not (Test-Path (Join-Path $testInstall $file))) { throw "安装缺少文件：$file" }
  }
}
$env:WHISTLEBOX_TEST_EXECUTABLE = Join-Path $testInstall 'whistle-box.exe'
node (Join-Path $PSScriptRoot '../audit/webview-smoke.mjs')
if ($LASTEXITCODE -ne 0) { throw '安装后的应用运行验证失败' }
$uninstaller = Get-ChildItem $testInstall -Filter '*uninstall*.exe' | Select-Object -First 1
if (-not $uninstaller) { throw '缺少卸载程序' }
$uninstallProcess = Start-Process -FilePath $uninstaller.FullName -ArgumentList @('/S', "_?=$testInstall") -PassThru -WindowStyle Hidden
if (-not $uninstallProcess.WaitForExit(120000)) { throw '卸载超时' }
if ($uninstallProcess.ExitCode -ne 0) { throw '卸载失败' }
if (Test-Path (Join-Path $testInstall 'whistle-box.exe')) { throw '卸载后应用仍然存在' }
Write-Output '通过：安装、覆盖升级、安装后真实界面和卸载。'
