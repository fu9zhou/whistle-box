param([Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Executable).Path
if ([IO.Path]::GetFileName($exe) -ne 'whistle-box.exe') { throw 'Select the installed whistle-box.exe.' }
$shutdown = Start-Process -FilePath $exe -ArgumentList '--shutdown','--remove-autostart' -WindowStyle Hidden -PassThru
if (-not $shutdown.WaitForExit(15000)) { throw 'Application did not finish cleanup. Exit it from the tray, then retry.' }
Start-Sleep -Seconds 3
$running = Get-Process -Name 'whistle-box' -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }
if ($running) { throw 'Application is still running. No files were changed.' }
$allowed = [IO.Path]::GetFullPath((Join-Path $env:APPDATA 'WhistleBox'))
$file = [IO.Path]::GetFullPath((Join-Path $allowed 'config.json'))
if ([IO.Path]::GetDirectoryName($file) -ne $allowed) { throw 'Invalid reset path.' }
if (Test-Path -LiteralPath $file) {
    $backup = Join-Path $allowed ('config.backup.' + [DateTime]::UtcNow.ToString('yyyyMMddHHmmssfff') + '.json')
    Move-Item -LiteralPath $file -Destination $backup
    Write-Host "Settings backed up: $backup"
}
Write-Host 'Reset complete. Rules, certificates and other Whistle installations are preserved.'
