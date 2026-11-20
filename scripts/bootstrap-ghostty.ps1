$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$version = '1.0.1'
# The package must match the Rust target architecture, not a possibly-emulated
# PowerShell process. An explicit override supports cross-build preparation.
$architecture = $env:RMUX_GHOSTTY_ARCH
if (-not $architecture) {
    $hostLine = (& rustc -vV | Select-String '^host:').ToString()
    if ($LASTEXITCODE -ne 0) { throw 'Run through mise with the project Rust toolchain' }
    $architecture = ($hostLine -split ' ')[1].Split('-')[0]
}
$hashes = @{
    'x86_64' = '8e2eb9e1822e85d786b456c8beb86f49d09c318c91f0c2913f91e7ffa12cda29'
    'aarch64' = 'cc61da5f968ea0b08639bdf019c6b7da46bad1d985774fd8a2de57af32862892'
}
if (-not $hashes.ContainsKey($architecture)) { throw 'Supported architectures: x86_64, aarch64' }
$package = "ghostty-vt-$version-$architecture-windows-gnu"
$deps = Join-Path (Get-Location) '.deps'
New-Item -ItemType Directory -Force $deps | Out-Null
$archive = Join-Path $deps "$package.zip"
if (-not (Test-Path $archive)) {
    Invoke-WebRequest "https://release.files.ghostty.org/libghostty-vt/$version/$package.zip" -OutFile "$archive.part"
    Move-Item "$archive.part" $archive
}
if ((Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant() -ne $hashes[$architecture]) {
    throw "Checksum mismatch: $archive"
}
Expand-Archive -Path $archive -DestinationPath $deps -Force
$source = Join-Path $deps $package
foreach ($file in @('include/ghostty/vt.h', 'lib/ghostty-vt.lib', 'bin/ghostty-vt.dll')) {
    if (-not (Test-Path (Join-Path $source $file))) { throw "Package missing $file" }
}
$destination = Join-Path $deps 'ghostty-vt'
New-Item -ItemType Directory -Force $destination | Out-Null
foreach ($directory in @('include', 'lib', 'bin')) {
    Copy-Item (Join-Path $source $directory) -Destination $destination -Recurse -Force
}
Write-Output "Ghostty VT $version ($architecture) ready at $destination"
