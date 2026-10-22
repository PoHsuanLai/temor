# temor shell integration for fish.
#
# Marks prompts and commands with OSC 133 (A prompt start, B prompt end, C output start,
# D;<status> command finished) and reports the working directory with OSC 7. fish finds this
# in vendor_conf.d through temor's XDG_DATA_DIRS entry; it never edits your files.
# TEMOR_SHELL_INTEGRATION=0 turns it off. fish 4 and newer emit these marks themselves, so
# there this file adds no marks of its own.
#
# It also draws temor's own prompt in place of fish's, whatever fish_prompt your config or theme
# defines: a blank line, the folder and the git branch, then an empty line to type on, with no
# right-side prompt. TEMOR_PROMPT=shell keeps your own prompt instead; TEMOR_PROMPT=temor (or
# unset) means temor's.

# Take temor's entry back out of XDG_DATA_DIRS, so programs started from this shell never see
# it. Only that entry goes: conf.d snippets that ran before this one may have added their own.
set -l _temor_entry (string replace -r '/fish/vendor_conf\.d/[^/]*$' '' -- (status filename))
set -l _temor_dirs (string split : -- "$XDG_DATA_DIRS")
if set -l _temor_at (contains -i -- $_temor_entry $_temor_dirs)
    set -e _temor_dirs[$_temor_at]
end
if set -q TEMOR_FISH_XDG_DATA_DIRS
    # It was set before temor started fish.
    set -e TEMOR_FISH_XDG_DATA_DIRS
    set -gx XDG_DATA_DIRS (string join : -- $_temor_dirs)
else if test "$_temor_dirs" = "/usr/local/share /usr/share"
    # It was unset, and nothing but temor's stand-in for the default is left.
    set -e XDG_DATA_DIRS
else
    set -gx XDG_DATA_DIRS (string join : -- $_temor_dirs)
end

# A pager that would take the whole screen (git, aws) prints into the block instead, where the
# output scrolls and is searched with the block. A value you set stays, and so does a pager git's own
# config names (delta, diff-so-fancy). TEMOR_PAGER=system leaves them to the system.
if test "$TEMOR_PAGER" != system
    if not set -q GIT_PAGER; and not command git config --get core.pager >/dev/null 2>&1
        set -gx GIT_PAGER cat
    end
    set -q AWS_PAGER; or set -gx AWS_PAGER ""
end
set -e _temor_entry _temor_dirs _temor_at

status is-interactive; or exit 0
test "$TEMOR_SHELL_INTEGRATION" = 0; and exit 0
set -q _temor_integrated; and exit 0
set -g _temor_integrated 1
set -g _temor_executing
set -g _temor_seen

# Prints the git branch of $PWD, or nothing outside a checkout. Reads .git/HEAD directly; no git
# process. A .git file (a worktree) names the real directory.
function _temor_branch
    set -l dir $PWD
    set -l gitdir
    while true
        if test -d $dir/.git
            set gitdir $dir/.git
            break
        else if test -f $dir/.git
            read -l line <$dir/.git 2>/dev/null
            string match -q 'gitdir: *' -- $line; or return 0
            set gitdir (string replace 'gitdir: ' '' -- $line)
            string match -q '/*' -- $gitdir; or set gitdir $dir/$gitdir
            break
        end
        test "$dir" = /; and return 0
        set dir (string replace -r '/[^/]*$' '' -- $dir)
        test -n "$dir"; or set dir /
    end
    test -r $gitdir/HEAD; or return 0
    read -l head <$gitdir/HEAD 2>/dev/null
    test -n "$head"; or return 0
    if string match -q 'ref: refs/heads/*' -- $head
        string replace 'ref: refs/heads/' '' -- $head
    else
        string sub -l 7 -- $head
    end
end

# temor's prompt, installed over fish's by _temor_install_prompt. Colours are ANSI
# indices, so the terminal's palette themes them.
function _temor_prompt
    set -l dir $PWD
    if test -n "$HOME" -a "$HOME" != /
        if test "$PWD" = "$HOME"
            set dir '~'
        else if string match -q -- "$HOME/*" $PWD
            set dir '~'(string sub -s (math (string length -- $HOME) + 1) -- $PWD)
        end
    end
    test -n "$_temor_seen"; and echo
    printf '\e[90m%s\e[39m' $dir
    set -l branch (_temor_branch)
    test -n "$branch"; and printf '  \e[35m%s\e[0m' $branch
    # The reset after the newline keeps fish from trimming the newline off the prompt.
    printf '\n\e[0m'
end

# Replaces fish_prompt (and any right-side prompt) with temor's, unless the person keeps their own.
function _temor_install_prompt
    test "$TEMOR_PROMPT" = shell; and return 0
    function fish_prompt
        _temor_prompt
    end
    function fish_right_prompt
    end
end

# Repaints (a resized window) draw the prompt again, so the blank line is tied to commands, not
# to drawing: there is one before every prompt once a command has run.
function _temor_command_seen --on-event fish_preexec
    set -g _temor_seen 1
end

if not string match -qr '^[0-3]\.' -- $version
    # fish 4 and newer draw the marks themselves; only the prompt is installed, once, when the
    # first prompt is about to be drawn (after config.fish has had its say).
    function _temor_first_prompt --on-event fish_prompt
        functions -e _temor_first_prompt
        _temor_install_prompt
    end
    exit 0
end


function _temor_prompt_start --on-event fish_prompt
    printf '\e]7;file://%s%s\e\\\\' $hostname (string escape --style=url -- $PWD)
    printf '\e]133;A\e\\\\'
    # A prompt defined in config.fish replaces any earlier one, so the end mark is wrapped
    # around the prompt function the first time a prompt is drawn.
    if not functions -q _temor_original_prompt
        _temor_install_prompt
        functions -c fish_prompt _temor_original_prompt
        function fish_prompt
            _temor_original_prompt
            printf '\e]133;B\e\\\\'
        end
    end
end

function _temor_preexec --on-event fish_preexec
    set -g _temor_executing 1
    printf '\e]133;C\e\\\\'
end

function _temor_postexec --on-event fish_postexec
    set -l status_ $status
    if test -n "$_temor_executing"
        printf '\e]133;D;%s\e\\\\' $status_
        set -g _temor_executing
    end
end

# A program that erases the scrollback (CSI 3 J) to redraw itself takes only its own rows; the
# blocks above it stay. Your own `clear` should take them all, so it says so to temor first
# (not `clear -x`, which keeps the scrollback).
# A `clear` you defined yourself is left alone.
if not functions -q clear
    function clear
        contains -- -x $argv; or printf '\e]7502;clear\a'
        command clear $argv
    end
end
