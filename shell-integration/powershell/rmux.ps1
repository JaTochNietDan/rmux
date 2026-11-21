# Opt-in PowerShell prompt integration. Dot-source after configuring your prompt.
# No profile edits, external commands, PSReadLine replacement or execution-policy changes.
if ($env:TERM_PROGRAM -ne 'rmux' -or $env:RMUX_SHELL_INTEGRATION -eq '0') { return }
if (Test-Path variable:global:_rmux_original_prompt) { return }

$global:_rmux_original_prompt = $function:prompt
function global:prompt {
    # Invoke the user's prompt before running our own commands. Do not replace
    # its implementation or report a synthetic command exit status.
    $promptText = & $global:_rmux_original_prompt
    $escape = [char]27
    $bell = [char]7
    $directory = ''
    $location = Get-Location
    if ($location.Provider.Name -eq 'FileSystem') {
        $uri = [Uri]::new($location.ProviderPath)
        if ($uri.IsFile) { $directory = "$escape]7;$($uri.AbsoluteUri)$bell" }
    }
    return "$directory$escape]133;A$bell$promptText$escape]133;B$bell"
}
