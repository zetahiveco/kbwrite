# Install the latest KbWrite release on Windows.
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$api = "https://api.github.com/repos/zetahiveco/kbwrite/releases/latest"
$headers = @{
  Accept     = "application/vnd.github+json"
  "User-Agent" = "kbwrite-install"
}

Write-Host "Looking up the latest KbWrite release..."
$release = Invoke-RestMethod -Uri $api -Headers $headers

$arm = $env:PROCESSOR_ARCHITECTURE -eq "ARM64" -or $env:PROCESSOR_ARCHITEW6432 -eq "ARM64"
$asset = $null
if ($arm) {
  $asset = $release.assets | Where-Object { $_.name -match '(?i)(aarch64|arm64).*setup\.exe$' } | Select-Object -First 1
}
if (-not $asset) {
  $asset = $release.assets | Where-Object { $_.name -match '(?i)x64-setup\.exe$' } | Select-Object -First 1
}
if (-not $asset) {
  throw "No Windows installer was found in the latest release."
}

$dest = Join-Path $env:TEMP $asset.name
Write-Host "Downloading $($asset.name)..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $dest -Headers @{ "User-Agent" = "kbwrite-install" }
Write-Host "Opening the installer..."
Start-Process -FilePath $dest
Write-Host "Installer started."
