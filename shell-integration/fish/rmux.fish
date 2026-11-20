# Loaded automatically for fish sessions started by rmux. Can also be sourced manually.
# Uses built-ins and named events; never replaces prompt or user hook functions.
status is-interactive; or return
test "$TERM_PROGRAM" = rmux; or return
test "$RMUX_SHELL_INTEGRATION" != 0; or return
set -q _rmux_fish_loaded; and return
set -g _rmux_fish_loaded 1
set -g _rmux_fish_running 0

function _rmux_fish_directory --on-variable PWD
    status is-command-substitution; and return
    set -l encoded (string escape --style=url -- "$PWD")
    printf '\e]7;file://localhost%s\a' "$encoded"
end

function _rmux_fish_prompt --on-event fish_prompt
    # Canceled/invalid commands may reach a new prompt without postexec.
    if test "$_rmux_fish_running" = 1
        printf '\e]133;D\a'
        set -g _rmux_fish_running 0
    end
    _rmux_fish_directory
    printf '\e]133;A\a'
end

function _rmux_fish_preexec --on-event fish_preexec
    set -g _rmux_fish_running 1
    printf '\e]133;C\a'
end

function _rmux_fish_postexec --on-event fish_postexec
    set -l command_status $status
    if test "$_rmux_fish_running" = 1
        printf '\e]133;D;%d\a' "$command_status"
        set -g _rmux_fish_running 0
    end
end

function _rmux_fish_posterror --on-event fish_posterror
    if test "$_rmux_fish_running" = 1
        printf '\e]133;D\a'
        set -g _rmux_fish_running 0
    end
end
