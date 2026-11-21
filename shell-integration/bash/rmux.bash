# Source from an interactive Bash session or ~/.bashrc. Bash 3.2+; no external commands.
[[ $- == *i* && ${TERM_PROGRAM-} == rmux ]] || return 0
[[ ${RMUX_SHELL_INTEGRATION-1} != 0 && -z ${_rmux_bash_loaded-} ]] || return 0

_rmux_bash_loaded=1
# Bash 4.4+ expands PS0 after reading a command and before executing it.
# Preserve user pre-command prompt content without installing a DEBUG trap.
# https://www.gnu.org/software/bash/manual/html_node/Bash-Variables.html
if (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 4) )); then
    PS0=$'\e]133;C\a'${PS0-}
fi
_rmux_bash_prompt_end=$'\\[\e]133;B\a\\]'

# Run before user prompt hooks, which may overwrite the command's exit status.
# Returning it also preserves the status seen by the first user hook.
_rmux_bash_capture_status() {
    _rmux_bash_command_status=$?
    return "$_rmux_bash_command_status"
}

_rmux_bash_precmd() {
    local command_status=${_rmux_bash_command_status-0}
    if [[ ${_rmux_bash_prompt_seen-} == 1 ]]; then
        builtin printf '\e]133;D;%d\a' "$command_status"
    fi
    _rmux_bash_prompt_seen=1
    local LC_ALL=C
    local encoded='' character hex byte index
    for (( index = 0; index < ${#PWD}; index++ )); do
        character=${PWD:index:1}
        case $character in
            [a-zA-Z0-9/._~-]) encoded+=$character ;;
            *) builtin printf -v byte '%d' "'$character"
               builtin printf -v hex '%02X' "$((byte & 255))"
               encoded+="%$hex" ;;
        esac
    done
    builtin printf '\e]7;file://localhost%s\a\e]133;A\a' "$encoded"
    PS1=${PS1%"$_rmux_bash_prompt_end"}$_rmux_bash_prompt_end
    return "$command_status"
}

# Preserve scalar hooks, including multiline commands and trailing comments.
# Modern Bash also supports PROMPT_COMMAND arrays; retain their element order.
case $(declare -p PROMPT_COMMAND 2>/dev/null) in
    'declare -a '*) PROMPT_COMMAND=(_rmux_bash_capture_status "${PROMPT_COMMAND[@]}" _rmux_bash_precmd) ;;
    *) PROMPT_COMMAND=$'_rmux_bash_capture_status\n'"${PROMPT_COMMAND-}"$'\n_rmux_bash_precmd' ;;
esac
