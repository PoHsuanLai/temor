# temor shell integration for zsh.
#
# Marks prompts and commands with OSC 133 (A prompt start, B prompt end, C output start,
# D;<status> command finished) and reports the working directory with OSC 7. temor loads this
# from its ZDOTDIR shim; it never edits your files. TEMOR_SHELL_INTEGRATION=0 turns it off.
#
# It also draws temor's own prompt in place of the shell's, whatever PS1 your .zshrc or theme sets:
# the folder and the git branch, then an empty line to type on, with no right-side prompt.
# TEMOR_PROMPT=shell keeps your own prompt instead; TEMOR_PROMPT=temor (or unset) means temor's.

[[ -o interactive ]] || return 0
[[ ${TEMOR_SHELL_INTEGRATION-} == 0 ]] && return 0
(( ${+_temor_integrated} )) && return 0
typeset -g _temor_integrated=1
typeset -g _temor_executing=
typeset -g _temor_ps1=
typeset -g _temor_seen=
typeset -g _temor_head=

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

# There is no dirty marker here on purpose. Whether the folder has uncommitted changes takes a
# `git status`, which would hold the prompt back in a large repository, and temor's chips on the
# same line already show it from a run that does not block the shell.
#
# Sets REPLY to the git branch of $PWD as "  <name>" in magenta, or empty outside a checkout.
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

# Whether temor draws the prompt: always, unless the person keeps their own.
_temor_prompt_is_ours() {
    [[ ${TEMOR_PROMPT-} != shell ]]
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
        branch="  %{"$'\e[35m'"%}$REPLY%{"$'\e[0m'"%}"
    fi
    # Without a terminal description that can move the cursor up (an unknown TERM), zsh draws
    # nothing at all for a prompt whose last line is empty. The folder line is then printed by
    # _temor_precmd ahead of the prompt, with the end mark, and the prompt is left empty, so the
    # typing line starts at the left edge and the folder line stays visible.
    local head="$blank%{"$'\e[90m'"%}$dir%{"$'\e[39m'"%}$branch"
    zmodload -F zsh/terminfo +p:terminfo 2>/dev/null
    if [[ -n ${terminfo[cuu1]-} ]]; then
        _temor_head=
        PS1="$head"$'\n'
    else
        _temor_head=$head
        PS1=
    fi
    PS1+=$'%{\e]133;B\e\\%}'
    _temor_ps1=$PS1
    # A theme's right-side prompt would sit on temor's typing line.
    RPS1=
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
    _temor_keep_line_init
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
            if [[ -n $_temor_head ]]; then
                # zsh draws no prompt here, not even the end mark inside it, so both are written.
                print -Pr -- "$_temor_head"
                printf '\e]133;B\e\\'
            fi
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

# An old-style `zle -N zle-line-init myfn` in the person's files replaces the widget that runs
# the hooks, and temor's hook with it. If that happened, hook again: their widget is kept and
# runs first. (Hooking a name that is already listed does nothing, so it is taken out first.)
_temor_keep_line_init() {
    emulate -L zsh
    zmodload -e zsh/zle || return 0
    [[ ${widgets[zle-line-init]-} == user:azhw:zle-line-init ]] && return 0
    add-zle-hook-widget -d zle-line-init _temor_line_init
    add-zle-hook-widget zle-line-init _temor_line_init
}

# Under an unknown TERM the folder line is printed by _temor_precmd, so a screen clear (Ctrl-L)
# would drop it. This clears by hand, prints the line again at the top and redraws the prompt.
# Only the stock widget is replaced; one the person rebound is left alone.
_temor_clear_screen() {
    if [[ -z ${_temor_head-} ]]; then
        zle .clear-screen
        return
    fi
    zle -I
    printf '\e[H\e[2J'
    print -Pr -- "$_temor_head"
    zle reset-prompt
}

# temor's offer to load this script into a shell that is already running first types one line
# that asks which shell it is. zsh calls this before it files a line in history; returning 1
# makes it keep that line out, and the hook is added next to the person's own, which still run.
_temor_addhistory() {
    [[ $1 == *"sh -c 'echo temor-shell:"* ]] && return 1
    return 0
}

autoload -Uz add-zsh-hook add-zle-hook-widget
# Line editing loads lazily at the first prompt; it is loaded here so the widget can be hooked.
zmodload zsh/zle 2>/dev/null && add-zle-hook-widget zle-line-init _temor_line_init
[[ ${widgets[clear-screen]-} == builtin ]] && zle -N clear-screen _temor_clear_screen
add-zsh-hook precmd _temor_save_status
add-zsh-hook precmd _temor_precmd
add-zsh-hook preexec _temor_preexec
add-zsh-hook zshaddhistory _temor_addhistory

# A program that erases the scrollback (CSI 3 J) to redraw itself takes only its own rows; the
# blocks above it stay. Your own `clear` should take them all, so it says so to temor first.
# A `clear` you defined yourself is left alone.
if (( ! $+functions[clear] && ! $+aliases[clear] )); then
    clear() {
        printf '\e]7502;clear\a'
        command clear "$@"
    }
fi
