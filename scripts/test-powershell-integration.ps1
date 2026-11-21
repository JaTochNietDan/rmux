$ErrorActionPreference = 'Stop'
$env:TERM_PROGRAM = 'rmux'
$env:RMUX_SHELL_INTEGRATION = '1'
$script:promptCalls = 0
function global:prompt { $script:promptSawSuccess = $?; $script:promptCalls++; return 'fixture> ' }
$integration = Join-Path $PSScriptRoot '../shell-integration/powershell/rmux.ps1'
$root = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
$directory = Join-Path $root 'project 世界 % space'
$original = Get-Location
try {
    New-Item -ItemType Directory -Path $directory | Out-Null
    Set-Location -LiteralPath $directory
    $env:RMUX_SHELL_INTEGRATION = '0'
    . $integration
    if (Test-Path variable:global:_rmux_original_prompt) { throw 'Opt-out was ignored' }
    $env:RMUX_SHELL_INTEGRATION = '1'
    . $integration
    . $integration
    $global:LASTEXITCODE = 17
    Get-Item -LiteralPath (Join-Path $root 'missing') -ErrorAction SilentlyContinue
    $result = prompt
    $escape = [char]27
    $bell = [char]7
    $expectedUri = [Uri]::new((Get-Location).ProviderPath).AbsoluteUri
    $expected = "$escape]7;$expectedUri$bell$escape]133;A${bell}fixture> $escape]133;B$bell"
    if ($result -cne $expected) { throw 'Directory or prompt boundary output differs' }
    if ($script:promptSawSuccess) { throw 'Original prompt lost the failed-command status' }
    if ($script:promptCalls -ne 1) { throw 'Repeated sourcing duplicated prompt execution' }
    if ($global:LASTEXITCODE -ne 17) { throw 'Native command exit code was changed' }
    if (!$expectedUri.Contains('%25') -or !$expectedUri.Contains('%20')) { throw 'Directory URI was not escaped' }
    Set-Location Env:
    $result = prompt
    if ($result.Contains("]7;")) { throw 'Non-filesystem provider emitted a file URI' }
    'PowerShell prompt integration checks passed'
} finally {
    Set-Location -LiteralPath $original.ProviderPath
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
