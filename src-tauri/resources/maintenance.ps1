param([Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
try {
    if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { exit 0 }
    $exe = [IO.Path]::GetFullPath($Executable)
    $sameName = @(Get-Process -Name 'whistle-box' -ErrorAction SilentlyContinue)
    foreach ($candidate in $sameName) {
        if (-not $candidate.Path) { throw 'Cannot verify a running WhistleBox process. Please exit it manually.' }
    }
    $owned = @($sameName | Where-Object { $_.Path -eq $exe })
    if ($owned.Count -eq 0) { exit 0 }
    $request = Start-Process -FilePath $exe -ArgumentList '--shutdown' -WindowStyle Hidden -PassThru
    if (-not $request.WaitForExit(15000)) { throw 'Shutdown request timed out.' }
    foreach ($candidate in $owned) {
        if (-not $candidate.WaitForExit(15000)) { throw 'Please exit WhistleBox from its tray menu, then retry (older versions cannot receive shutdown requests).' }
    }
    exit 0
} catch { [Console]::Error.WriteLine($_.Exception.Message); exit 1 }
