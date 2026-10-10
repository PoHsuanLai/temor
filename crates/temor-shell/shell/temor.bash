# temor shell integration for bash (bash 4.4 or newer for the command mark).
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
_temor_ps1='${_temor_blank}\[\e[2m\]${_temor_pwd}\[\e[22m\]${_temor_branch}\n'
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

# Sets _temor_branch to "  <name>" (dim magenta) when $PWD is inside a git checkout, else empty.
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
    _temor_branch=$'  \001\e[2;35m\002'$head$'\001\e[0m\002'
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
    [[ ${PS0-} == *'_temor_executing=1'* ]] ||
        PS0="${PS0-}"'${_temor_mark[_temor_executing=1]-}\e]133;C\e\\'
    return "$_temor_status"
}

if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a'* ]]; then
    PROMPT_COMMAND=(_temor_save_status "${PROMPT_COMMAND[@]}" _temor_prompt)
else
    # Joined with newlines, not "; ": a value ending in ";" or a newline stays valid.
    PROMPT_COMMAND=$'_temor_save_status\n'"${PROMPT_COMMAND-}"$'\n_temor_prompt'
fi
