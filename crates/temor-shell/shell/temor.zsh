# temor shell integration for zsh.
#
# Marks prompts and commands with OSC 133 (A prompt start, B prompt end, C output start,
# D;<status> command finished) and reports the working directory with OSC 7. temor loads this
# from its ZDOTDIR shim; it never edits your files. TEMOR_SHELL_INTEGRATION=0 turns it off.
#
# It also replaces zsh's stock prompt (PS1 unset or exactly `%m%# `) with a clean two-line one:
# the folder and the git branch, then an empty line to type on. A prompt you set yourself is never touched.
# TEMOR_PROMPT=shell keeps the shell's own prompt even when it is the stock one; TEMOR_PROMPT=temor
# (or unset) means "temor's when the shell's is stock".

[[ -o interactive ]] || return 0
[[ ${TEMOR_SHELL_INTEGRATION-} == 0 ]] && return 0
(( ${+_temor_integrated} )) && return 0
typeset -g _temor_integrated=1
typeset -g _temor_executing=
typeset -g _temor_ps1=
typeset -g _temor_seen=

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

# Sets REPLY to the git branch of $PWD as "  <name>" in dim magenta, or empty outside a checkout.
# Reads .git/HEAD directly; no git process. A .git file (a worktree) names the real directory.
_temor_find_branch() {
    emulate -L zsh
    local dir=$PWD gitdir= line= head=
    REPLY=
    while true; do
        if [[ -d $dir/.git ]]; then
            gitdir=$dir/.git
            break
        elif [[ -f $dir/.git ]]; then
            read -r line <"$dir/.git" 2>/dev/null
            [[ $line == 'gitdir: '* ]] || return 0
            gitdir=${line#gitdir: }
            [[ $gitdir == /* ]] || gitdir=$dir/$gitdir
            break
        fi
        [[ -z $dir || $dir == / ]] && return 0
        dir=${dir:h}
    done
    read -r head <"$gitdir/HEAD" 2>/dev/null
    [[ -n $head ]] || return 0
    if [[ $head == 'ref: refs/heads/'* ]]; then
        head=${head#ref: refs/heads/}
    else
        head=${head[1,7]}
    fi
    REPLY=$head
}

# Escapes text so it is shown literally in a prompt.
_temor_prompt_text() {
    emulate -L zsh
    REPLY=${1//\%/%%}
    if [[ -o promptsubst ]]; then
        REPLY=${REPLY//(#m)[\\\$\`]/\\$MATCH}
    fi
}

# Whether the prompt is zsh's own stock one, or the one temor put there last time.
_temor_prompt_is_ours() {
    [[ ${TEMOR_PROMPT-} != shell ]] || return 1
    [[ -z ${PS1-} || $PS1 == '%m%# ' || $PS1 == "$_temor_ps1" ]]
}

# Builds temor's prompt: a blank line (not before the first prompt), the folder and the
# branch. Colours are ANSI indices, so the terminal's palette themes them. The end mark is
# part of it, so _temor_precmd has nothing to add.
_temor_build_prompt() {
    emulate -L zsh
    local dir=$PWD branch= blank=
    [[ -n $_temor_seen ]] && blank=$'\n'
    if [[ -n ${HOME-} && $HOME != / && ( $PWD == $HOME || $PWD == $HOME/* ) ]]; then
        dir="~${PWD#$HOME}"
    fi
    local REPLY
    _temor_prompt_text $dir
    dir=$REPLY
    _temor_find_branch
    if [[ -n $REPLY ]]; then
        _temor_prompt_text $REPLY
        branch="  %{"$'\e[2;35m'"%}$REPLY%{"$'\e[0m'"%}"
    fi
    # Without a terminal description that can move the cursor up (an unknown TERM), zsh draws
    # no prompt at all when its last line is empty, so that line then starts with a space.
    local typing=
    zmodload -F zsh/terminfo +p:terminfo 2>/dev/null
    [[ -n ${terminfo[cuu1]-} ]] || typing=' '
    PS1="$blank%{"$'\e[2m'"%}$dir%{"$'\e[22m'"%}$branch"$'\n'"$typing"
    PS1+=$'%{\e]133;B\e\\%}'
    _temor_ps1=$PS1
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
        if _temor_prompt_is_ours; then
            _temor_build_prompt
            _temor_seen=1
        fi
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
