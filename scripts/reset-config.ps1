[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$PSDefaultParameterValues['Out-File:Encoding'] = 'utf8'

$configDir = Join-Path $env:APPDATA "WhistleBox"
$configFile = Join-Path $configDir "config.json"

$proc = Get-Process -Name "whistle-box" -ErrorAction SilentlyContinue
if ($proc) {
    Write-Host "[*] Closing WhistleBox..." -ForegroundColor Yellow
    $proc | Stop-Process -Force
    Start-Sleep -Seconds 1
}

$conn = Get-NetTCPConnection -LocalPort 18899 -State Listen -ErrorAction SilentlyContinue
if ($conn) {
    foreach ($c in $conn) {
        $p = Get-Process -Id $c.OwningProcess -ErrorAction SilentlyContinue
        if ($p -and ($p.ProcessName -match 'node|whistle-box')) {
            Write-Host "[*] Killing $($p.ProcessName) on port 18899 (PID: $($p.Id))..." -ForegroundColor Yellow
            Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        }
    }
}

if (Test-Path $configFile) {
    Remove-Item $configFile -Force
    Write-Host "[OK] Config deleted: $configFile" -ForegroundColor Green
} else {
    Write-Host "[--] Config file not found, nothing to delete" -ForegroundColor Gray
}

$whistleDataDir = Join-Path $env:USERPROFILE ".WhistleBoxData"
if (Test-Path $whistleDataDir) {
    Remove-Item $whistleDataDir -Recurse -Force
    Write-Host "[OK] Whistle data dir cleaned: $whistleDataDir" -ForegroundColor Green
}

Write-Host ""
Write-Host "Reset complete. Restart WhistleBox to see the setup wizard." -ForegroundColor Cyan
Write-Host ""
