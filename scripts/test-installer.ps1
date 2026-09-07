$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
  throw '安装验证仅允许在 GitHub 托管的临时 Windows 环境运行。'
}
$version = (Get-Content (Join-Path $PSScriptRoot '../package.json') -Raw | ConvertFrom-Json).version
$installer = Get-Item (Join-Path $PSScriptRoot "../release/WhistleBox_${version}_x64-setup.exe")
$expectedNode = (Get-FileHash (Join-Path $PSScriptRoot '../src-tauri/binaries/node-x86_64-pc-windows-msvc.exe')).Hash
$expectedWhistle = (Get-Content (Join-Path $PSScriptRoot '../src-tauri/resources/whistle/package.json') -Raw | ConvertFrom-Json).dependencies.whistle

function Install-Package([string]$Setup, [string]$Destination) {
  $installProcess = Start-Process -FilePath $Setup -ArgumentList @('/S', "/D=$Destination") -PassThru -WindowStyle Hidden
  if (-not $installProcess.WaitForExit(120000)) { throw '安装或升级超时。' }
  if ($installProcess.ExitCode -ne 0) { throw "安装失败：$($installProcess.ExitCode)" }
}
function Assert-InstalledPayload([string]$Destination) {
  foreach ($file in @('whistle-box.exe','node.exe','resources/whistle/launcher.cjs','resources/maintenance.ps1')) {
    if (-not (Test-Path (Join-Path $Destination $file))) { throw "安装缺少文件：$file" }
  }
  if ((Get-Item (Join-Path $Destination 'whistle-box.exe')).VersionInfo.ProductVersion -ne $version) { throw '程序版本不匹配。' }
  if ((Get-FileHash (Join-Path $Destination 'node.exe')).Hash -ne $expectedNode) { throw 'Node 运行时不匹配。' }
  $installedWhistle = (Get-Content (Join-Path $Destination 'resources/whistle/node_modules/whistle/package.json') -Raw | ConvertFrom-Json).version
  if ($installedWhistle -ne $expectedWhistle) { throw 'Whistle 版本不匹配。' }
}
function Test-ApplicationAndUninstall([string]$Destination) {
  $env:WHISTLEBOX_TEST_EXECUTABLE = Join-Path $Destination 'whistle-box.exe'
  try {
    node (Join-Path $PSScriptRoot '../audit/webview-smoke.mjs')
    if ($LASTEXITCODE -ne 0) { throw '安装后的应用运行验证失败。' }
  } finally { Remove-Item Env:WHISTLEBOX_TEST_EXECUTABLE -ErrorAction SilentlyContinue }
  $uninstaller = Get-ChildItem $Destination -Filter '*uninstall*.exe' | Select-Object -First 1
  if (-not $uninstaller) { throw '缺少卸载程序。' }
  $uninstallProcess = Start-Process -FilePath $uninstaller.FullName -ArgumentList @('/S', "_?=$Destination") -PassThru -WindowStyle Hidden
  if (-not $uninstallProcess.WaitForExit(120000)) { throw '卸载超时。' }
  if ($uninstallProcess.ExitCode -ne 0) { throw '卸载失败。' }
  if (Test-Path (Join-Path $Destination 'whistle-box.exe')) { throw '卸载后应用仍然存在。' }
}

$baselineDir = Join-Path $env:RUNNER_TEMP 'WhistleBox-baseline'
New-Item -ItemType Directory -Path $baselineDir -Force | Out-Null
gh release download v0.1.0 --repo fu9zhou/whistle-box --pattern 'WhistleBox_0.1.0_x64-setup.exe' --dir $baselineDir --clobber
$downloadExitCode = $LASTEXITCODE
Remove-Item Env:GH_TOKEN -ErrorAction SilentlyContinue
if ($downloadExitCode) { throw '无法下载已发布的 0.1.0 升级基线。' }
$upgradeDir = Join-Path $env:RUNNER_TEMP 'WhistleBox-upgrade-test'
Install-Package (Join-Path $baselineDir 'WhistleBox_0.1.0_x64-setup.exe') $upgradeDir
if ((Get-Item (Join-Path $upgradeDir 'whistle-box.exe')).VersionInfo.ProductVersion -ne '0.1.0') { throw '升级基线版本不匹配。' }
foreach ($attempt in 1..2) {
  Install-Package $installer.FullName $upgradeDir
  Assert-InstalledPayload $upgradeDir
}
Test-ApplicationAndUninstall $upgradeDir

$freshDir = Join-Path $env:RUNNER_TEMP 'WhistleBox-fresh-test'
Install-Package $installer.FullName $freshDir
Assert-InstalledPayload $freshDir
Test-ApplicationAndUninstall $freshDir
Write-Output '通过：0.1.0 升级、同版本覆盖、全新安装、运行时完整性、安装后真实界面和卸载。'
