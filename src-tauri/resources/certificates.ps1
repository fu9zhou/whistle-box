$ErrorActionPreference = 'Stop'
$request = [Console]::In.ReadToEnd() | ConvertFrom-Json
$bytes = [Convert]::FromBase64String($request.certificate)
$text = [Text.Encoding]::UTF8.GetString($bytes)
if ($text.Contains('-----BEGIN CERTIFICATE-----')) {
  $base64 = $text.Replace('-----BEGIN CERTIFICATE-----','').Replace('-----END CERTIFICATE-----','') -replace '\s',''
  $bytes = [Convert]::FromBase64String($base64)
}
$cert = [Security.Cryptography.X509Certificates.X509Certificate2]::new($bytes)
$store = [Security.Cryptography.X509Certificates.X509Store]::new('Root','CurrentUser')
$flags = if ($request.action -eq 'status') { 'ReadOnly' } else { 'ReadWrite' }
try {
  $store.Open($flags)
  $matches = $store.Certificates.Find('FindByThumbprint',$cert.Thumbprint,$false)
  $exists = $matches.Count -gt 0
  $added = $false
  if ($request.action -eq 'install' -and -not $exists) { $store.Add($cert); $added = $true }
  if ($request.action -eq 'remove') {
    if ($request.ownedThumbprint -ne $cert.Thumbprint) { throw 'Certificate ownership mismatch' }
    foreach ($match in $matches) { $store.Remove($match) }
  }
  @{ thumbprint=$cert.Thumbprint; exists=$exists; added=$added } | ConvertTo-Json -Compress
} finally { $store.Close(); $cert.Dispose() }
