# Source from an interactive Bash session or ~/.bashrc. Bash 3.2+; no external commands.
[[ $- == *i* && ${TERM_PROGRAM-} == rmux ]] || return 0
[[ ${RMUX_SHELL_INTEGRATION-1} != 0 && -z ${_rmux_bash_loaded-} ]] || return 0

_rmux_bash_loaded=1
_rmux_bash_prompt_end=$'\\[\e]133;B\a\\]'

_rmux_bash_precmd() {
    local command_status=$?
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
    'declare -a '*) PROMPT_COMMAND+=(_rmux_bash_precmd) ;;
    *) PROMPT_COMMAND="${PROMPT_COMMAND-}"$'\n_rmux_bash_precmd' ;;
esac
