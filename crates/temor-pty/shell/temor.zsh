# temor shell integration for zsh.
#
# Marks prompts and commands with OSC 133 (A prompt start, B prompt end, C output start,
# D;<status> command finished) and reports the working directory with OSC 7. temor loads this
# from its ZDOTDIR shim; it never edits your files. TEMOR_SHELL_INTEGRATION=0 turns it off.

[[ -o interactive ]] || return 0
[[ ${TEMOR_SHELL_INTEGRATION-} == 0 ]] && return 0
(( ${+_temor_integrated} )) && return 0
typeset -g _temor_integrated=1
typeset -g _temor_executing=

# Percent-encodes a path for a file:// URL, byte by byte, into REPLY.
_temor_encode_cwd() {
    emulate -L zsh
    local LC_ALL=C dir=$1 out= ch hex i
    for (( i = 1; i <= ${#dir}; i++ )); do
        ch=${dir[i]}
        if [[ $ch == [a-zA-Z0-9/._~-] ]]; then
            out+=$ch
        else
            printf -v hex '%02X' "'$ch"
            out+=%${hex: -2}
        fi
    done
    REPLY=$out
}

_temor_precmd() {
    local status_=$?
    if [[ -n $_temor_executing ]]; then
        printf '\e]133;D;%s\e\\' "$status_"
        _temor_executing=
    fi
    local REPLY
    _temor_encode_cwd "$PWD"
    printf '\e]7;file://%s%s\e\\' "${HOST-}" "$REPLY"
    printf '\e]133;A\e\\'
    # Startup files and themes may rewrite PS1 at any time, so the mark is put back before
    # every prompt.
    [[ $PS1 == *$'\e]133;B'* ]] || PS1+=$'%{\e]133;B\e\\%}'
}

_temor_preexec() {
    _temor_executing=1
    printf '\e]133;C\e\\'
}

autoload -Uz add-zsh-hook
add-zsh-hook precmd _temor_precmd
add-zsh-hook preexec _temor_preexec
