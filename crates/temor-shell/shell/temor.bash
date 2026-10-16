# temor shell integration for bash. The command mark comes from PS0 (bash 4.4 or newer), or
# from a DEBUG trap on older bash, as Warp's bash-preexec does.
#
# Marks prompts and commands with OSC 133 (A prompt start, B prompt end, C output start,
# D;<status> command finished) and reports the working directory with OSC 7. temor sources this
# after your own startup files; it never edits them. TEMOR_SHELL_INTEGRATION=0 turns it off.
#
# It also draws temor's own prompt in place of the shell's, whatever PS1 your startup files set:
# a blank line, the folder and the git branch, then an empty line to type on. TEMOR_PROMPT=shell
# keeps your own prompt instead; TEMOR_PROMPT=temor (or unset) means temor's.

[[ $- == *i* ]] || return 0
[[ ${TEMOR_SHELL_INTEGRATION-} == 0 ]] && return 0
[[ -n ${_temor_integrated-} ]] && return 0
_temor_integrated=1
_temor_executing=
_temor_status=0
_temor_cwd=
_temor_cwd_url=
_temor_pwd=
_temor_branch=
_temor_blank=
_temor_seen=
# temor's prompt. The parts that change each prompt are variables, expanded by bash when it
# draws PS1; colours are ANSI indices, so the terminal's palette themes them.
_temor_ps1='${_temor_blank}\[\e[90m\]${_temor_pwd}\[\e[39m\]${_temor_branch}\n'
_temor_ps1+='\[\e]133;B\e\\\]'

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

# Sets _temor_branch to "  <name>" (magenta) when $PWD is inside a git checkout, else empty.
# Reads .git/HEAD directly; no git process. A .git file (a worktree) names the real directory.
_temor_find_branch() {
    _temor_branch=
    local dir=$PWD gitdir= line= head=
    while :; do
        if [[ -d $dir/.git ]]; then
            gitdir=$dir/.git
            break
        elif [[ -f $dir/.git ]]; then
            read -r line <"$dir/.git" 2>/dev/null || :
            [[ $line == 'gitdir: '* ]] || return 0
            gitdir=${line#gitdir: }
            [[ $gitdir == /* ]] || gitdir=$dir/$gitdir
            break
        fi
        [[ -z $dir || $dir == / ]] && return 0
        dir=${dir%/*}
        dir=${dir:-/}
    done
    read -r head <"$gitdir/HEAD" 2>/dev/null || :
    [[ -n $head ]] || return 0
    if [[ $head == 'ref: refs/heads/'* ]]; then
        head=${head#ref: refs/heads/}
    else
        head=${head:0:7}
    fi
    # \001 and \002 tell readline the colour codes take no room.
    _temor_branch=$'  \001\e[35m\002'$head$'\001\e[0m\002'
}

# Whether temor draws the prompt: always, unless the person keeps their own.
_temor_prompt_is_ours() {
    [[ ${TEMOR_PROMPT-} != shell ]]
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
    if _temor_prompt_is_ours; then
        _temor_blank=${_temor_seen:+$'\n'}
        _temor_seen=1
        _temor_pwd=$PWD
        if [[ -n ${HOME-} && $HOME != / && ( $PWD == "$HOME" || $PWD == "$HOME"/* ) ]]; then
            _temor_pwd="~${PWD#"$HOME"}"
        fi
        _temor_find_branch
        PS1=$_temor_ps1
    fi
    # Startup files and prompt frameworks may rewrite PS1 and PS0 at any time, so the marks
    # are put back before every prompt.
    # Every expansion has a default, so this stays quiet under `set -u`.
    [[ ${PS1-} == *'\e]133;B'* ]] || PS1="${PS1-}"'\[\e]133;B\e\\\]'
    if [[ -n ${_temor_debug_trap-} ]]; then
        _temor_ready=1
    else
        [[ ${PS0-} == *'_temor_executing=1'* ]] ||
            PS0="${PS0-}"'${_temor_mark[_temor_executing=1]-}\e]133;C\e\\'
    fi
    return "$_temor_status"
}

# bash before 4.4 has no PS0, so the output mark comes from a DEBUG trap, which runs before
# every simple command. The prompt sets _temor_ready and the first command of the typed line
# takes it, so a pipeline or a list is marked once; an empty line reaches PROMPT_COMMAND's
# first command instead, and completion is left alone.
_temor_preexec() {
    [[ -n ${COMP_LINE-} || -z ${_temor_ready-} ]] && return 0
    _temor_ready=
    [[ $BASH_COMMAND == _temor_save_status ]] && return 0
    _temor_executing=1
    printf '\e]133;C\e\\'
}

# The trap is installed by the first prompt, from PROMPT_COMMAND, and not here: bash 3.2 hides the
# shell's DEBUG trap inside a sourced file or a function, and puts it back when one sets its own.
# A DEBUG trap set before then (in the person's startup files) keeps running, first, so it sees
# the command's own $? and $_. One set later replaces temor's, as with any other DEBUG trap.
_temor_install_debug='[[ -n ${_temor_debug_trap-} ]] || {
_temor_debug_trap=$(trap -p DEBUG)
if [[ -n $_temor_debug_trap ]]; then
_temor_debug_trap=${_temor_debug_trap#"trap -- "}
eval "_temor_debug_trap=${_temor_debug_trap%" DEBUG"}"
trap '\''eval "$_temor_debug_trap"; _temor_preexec'\'' DEBUG
else
_temor_debug_trap=:
trap _temor_preexec DEBUG
fi
}
_temor_keep_status'

_temor_keep_status() {
    return "$_temor_status"
}

if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a'* ]]; then
    PROMPT_COMMAND=(_temor_save_status "${PROMPT_COMMAND[@]}" _temor_prompt)
elif ((BASH_VERSINFO[0] < 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] < 4))); then
    PROMPT_COMMAND=$'_temor_save_status\n'"$_temor_install_debug"$'\n'"${PROMPT_COMMAND-}"$'\n_temor_prompt'
else
    # Joined with newlines, not "; ": a value ending in ";" or a newline stays valid.
    PROMPT_COMMAND=$'_temor_save_status\n'"${PROMPT_COMMAND-}"$'\n_temor_prompt'
fi

# A program that erases the scrollback (CSI 3 J) to redraw itself takes only its own rows; the
# blocks above it stay. Your own `clear` should take them all, so it says so to temor first
# (not `clear -x`, which keeps the scrollback).
# A `clear` you defined yourself is left alone.
if ! declare -F clear >/dev/null && ! alias clear >/dev/null 2>&1; then
    clear() {
        [[ " $* " == *' -x '* ]] || printf '\e]7502;clear\a'
        command clear "$@"
    }
fi
