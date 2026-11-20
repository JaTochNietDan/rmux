param([ValidateSet('debug', 'release')][string]$Profile = 'release')
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if ($Profile -eq 'release') { & cargo build --release -p rmux-desktop }
else { & cargo build -p rmux-desktop }
if ($LASTEXITCODE -ne 0) { throw 'Rust build failed' }
$library = if ($env:GHOSTTY_VT_DIR) { $env:GHOSTTY_VT_DIR } else { '.deps/ghostty-vt' }
$destination = "target/$Profile/rmux-windows"
New-Item -ItemType Directory -Force $destination | Out-Null
Copy-Item "target/$Profile/rmux.exe" $destination -Force
Copy-Item (Join-Path $library 'bin/ghostty-vt.dll') $destination -Force
if (Test-Path (Join-Path $library 'bin/ghostty-vt.pdb')) {
    Copy-Item (Join-Path $library 'bin/ghostty-vt.pdb') $destination -Force
}
Write-Output (Join-Path (Get-Location) $destination)
