# devsweep one-line installer (Windows PowerShell 5.1+)
#
#   irm https://raw.githubusercontent.com/onion3130/devsweep/main/install.ps1 | iex
#
# Downloads the latest devsweep.exe release into %LOCALAPPDATA%\devsweep,
# adds it to your user PATH, and verifies it runs. Re-run anytime to update.
$ErrorActionPreference = 'Stop'

$Repo       = 'onion3130/devsweep'
$InstallDir = Join-Path $env:LOCALAPPDATA 'devsweep'
$ExePath    = Join-Path $InstallDir 'devsweep.exe'

Write-Host 'Installing devsweep...' -ForegroundColor Cyan

$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
$asset = @($release.assets | Where-Object { $_.name -like '*windows*.exe' })[0]
if (-not $asset) {
    throw 'No Windows binary found in the latest release. See https://github.com/' + $Repo + '/releases'
}

New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $ExePath

# Verify the download against the release's SHA-256 checksums.
$sumAsset = @($release.assets | Where-Object { $_.name -eq 'SHA256SUMS.txt' })[0]
if (-not $sumAsset) {
    Remove-Item $ExePath -Force -ErrorAction SilentlyContinue
    throw 'Release has no SHA256SUMS.txt — refusing to install an unverified binary.'
}
$sumFile = Join-Path $InstallDir 'SHA256SUMS.txt'
Invoke-WebRequest -Uri $sumAsset.browser_download_url -OutFile $sumFile
$expected = @(Get-Content $sumFile |
    Where-Object { $_ -match [regex]::Escape($asset.name) } |
    ForEach-Object { ($_ -split '\s+')[0] })[0]
if (-not $expected) {
    Remove-Item $ExePath -Force -ErrorAction SilentlyContinue
    throw 'Checksum entry missing for the downloaded file — refusing to install.'
}
$actual = (Get-FileHash -Path $ExePath -Algorithm SHA256).Hash
if ($actual -ne $expected.ToUpperInvariant()) {
    Remove-Item $ExePath -Force -ErrorAction SilentlyContinue
    throw 'Checksum mismatch — the download may be corrupt or tampered with. Aborted.'
}
Write-Host 'Checksum OK.' -ForegroundColor Green

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$InstallDir", 'User')
    Write-Host "Added $InstallDir to your user PATH." -ForegroundColor Green
}
$env:Path = "$env:Path;$InstallDir"

& $ExePath --version
Write-Host ''
Write-Host 'Done! Restart your terminal, then try:' -ForegroundColor Green
Write-Host '  devsweep scan C:\code'
Write-Host '  devsweep interactive C:\code'
