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

# Runs first in every precmd cycle (it is hooked before your .zshrc runs), so it sees the
# command's status before any other hook changes it.
_temor_save_status() {
    typeset -g _temor_status=$?
    typeset -g _temor_marked=
}

# Runs last: themes (powerlevel10k, pure, oh-my-zsh) rebuild PS1 in their own precmd hooks,
# so on every cycle this hook moves itself to the end of precmd_functions, where the mark it
# adds to PS1 survives them.
_temor_precmd() {
    if [[ ${precmd_functions[-1]-} != _temor_precmd ]]; then
        precmd_functions=(${precmd_functions:#_temor_precmd} _temor_precmd)
    fi
    if [[ -z ${_temor_marked-} ]]; then
        _temor_marked=1
        if [[ -n ${_temor_executing-} ]]; then
            printf '\e]133;D;%s\e\\' "${_temor_status-0}"
            _temor_executing=
        fi
        local REPLY
        _temor_encode_cwd "$PWD"
        printf '\e]7;file://%s%s\e\\' "${HOST-}" "$REPLY"
        printf '\e]133;A\e\\'
    fi
    [[ ${PS1-} == *$'\e]133;B'* ]] || PS1+=$'%{\e]133;B\e\\%}'
}

_temor_preexec() {
    _temor_executing=1
    printf '\e]133;C\e\\'
}

# zsh runs a snapshot of precmd_functions, so on the very first prompt a theme hooked after
# this script still runs after _temor_precmd. If PS1 lost the mark that way, it is written
# when line editing starts, which is exactly where the prompt ends.
_temor_line_init() {
    [[ ${PS1-} == *$'\e]133;B'* ]] || printf '\e]133;B\e\\'
}

autoload -Uz add-zsh-hook add-zle-hook-widget
# Line editing loads lazily at the first prompt; it is loaded here so the widget can be hooked.
zmodload zsh/zle 2>/dev/null && add-zle-hook-widget zle-line-init _temor_line_init
add-zsh-hook precmd _temor_save_status
add-zsh-hook precmd _temor_precmd
add-zsh-hook preexec _temor_preexec
