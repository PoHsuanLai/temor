# temor shell integration for fish.
#
# Marks prompts and commands with OSC 133 (A prompt start, B prompt end, C output start,
# D;<status> command finished) and reports the working directory with OSC 7. fish finds this
# in vendor_conf.d through temor's XDG_DATA_DIRS entry; it never edits your files.
# TEMOR_SHELL_INTEGRATION=0 turns it off. fish 4 and newer emit these marks themselves, so
# there this file only tidies the environment.

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
set -e _temor_entry _temor_dirs _temor_at

status is-interactive; or exit 0
test "$TEMOR_SHELL_INTEGRATION" = 0; and exit 0
set -q _temor_integrated; and exit 0
string match -qr '^[0-3]\.' -- $version; or exit 0
set -g _temor_integrated 1
set -g _temor_executing

function _temor_prompt_start --on-event fish_prompt
    printf '\e]7;file://%s%s\e\\\\' $hostname (string escape --style=url -- $PWD)
    printf '\e]133;A\e\\\\'
    # A prompt defined in config.fish replaces any earlier one, so the end mark is wrapped
    # around the prompt function the first time a prompt is drawn.
    if not functions -q _temor_original_prompt
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
