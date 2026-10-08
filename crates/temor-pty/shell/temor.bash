# temor shell integration for bash (bash 4.4 or newer for the command mark).
#
# Marks prompts and commands with OSC 133 (A prompt start, B prompt end, C output start,
# D;<status> command finished) and reports the working directory with OSC 7. temor sources this
# after your own startup files; it never edits them. TEMOR_SHELL_INTEGRATION=0 turns it off.

[[ $- == *i* ]] || return 0
[[ ${TEMOR_SHELL_INTEGRATION-} == 0 ]] && return 0
[[ -n ${_temor_integrated-} ]] && return 0
_temor_integrated=1
_temor_executing=
_temor_status=0
_temor_cwd=
_temor_cwd_url=

# Percent-encodes a path for a file:// URL, byte by byte, into _temor_cwd_url.
_temor_encode_cwd() {
    local LC_ALL=C path=$1 out= ch i
    for ((i = 0; i < ${#path}; i++)); do
        ch=${path:i:1}
        case $ch in
            [a-zA-Z0-9/._~-]) out+=$ch ;;
            *) printf -v ch '%02X' "'$ch"; out+=%${ch: -2} ;;
        esac
    done
    _temor_cwd_url=$out
}

_temor_save_status() {
    _temor_status=$?
    return "$_temor_status"
}

_temor_prompt() {
    if [[ -n $_temor_executing ]]; then
        printf '\e]133;D;%s\e\\' "$_temor_status"
        _temor_executing=
    fi
    if [[ $PWD != "$_temor_cwd" ]]; then
        _temor_cwd=$PWD
        _temor_encode_cwd "$PWD"
    fi
    printf '\e]7;file://%s%s\e\\' "${HOSTNAME-}" "$_temor_cwd_url"
    printf '\e]133;A\e\\'
    # Startup files and prompt frameworks may rewrite PS1 and PS0 at any time, so the marks
    # are put back before every prompt.
    [[ $PS1 == *'\e]133;B'* ]] || PS1+='\[\e]133;B\e\\\]'
    [[ $PS0 == *'_temor_executing=1'* ]] || PS0+='${_temor_mark[_temor_executing=1]}\e]133;C\e\\'
    return "$_temor_status"
}

if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a'* ]]; then
    PROMPT_COMMAND=(_temor_save_status "${PROMPT_COMMAND[@]}" _temor_prompt)
else
    PROMPT_COMMAND="_temor_save_status${PROMPT_COMMAND:+; $PROMPT_COMMAND}; _temor_prompt"
fi
