# temor shell integration for nushell.
#
# nushell 0.9x and newer can mark prompts and commands itself: osc133 (prompt start, input start,
# command start and command end with its exit status) and osc7 (the working directory). temor
# turns both on here, from a vendor autoload file in its own folder, so your config.nu is never
# edited. nushell reads this file before your config. TEMOR_SHELL_INTEGRATION=0 turns it off.
if ($env.TEMOR_SHELL_INTEGRATION? != "0") {
    $env.config.shell_integration.osc133 = true
    $env.config.shell_integration.osc7 = true
}

# It also draws temor's own prompt in place of nushell's, whatever your config.nu sets: a blank
# line, the folder and the git branch, then an empty line to type on, with no right-side prompt.
# TEMOR_PROMPT=shell keeps your own prompt instead; TEMOR_PROMPT=temor (or unset) means temor's.
# Colours are ANSI indices, so the terminal's palette themes them.
if ($env.TEMOR_SHELL_INTEGRATION? != "0") and ($env.TEMOR_PROMPT? != "shell") {
    # The git branch of $PWD, or nothing outside a checkout. Reads .git/HEAD directly; no git
    # process. A .git file (a worktree) names the real directory.
    def _temor_branch [] {
        mut dir = $env.PWD
        mut gitdir = ""
        loop {
            let dotgit = ($dir | path join ".git")
            match ($dotgit | path type) {
                "dir" => { $gitdir = $dotgit; break }
                "file" => {
                    let line = (open --raw $dotgit | lines | first | default "")
                    if not ($line | str starts-with "gitdir: ") { return "" }
                    $gitdir = ($dir | path join ($line | str replace "gitdir: " ""))
                    break
                }
                _ => {}
            }
            let parent = ($dir | path dirname)
            if $parent == $dir { return "" }
            $dir = $parent
        }
        let head = (try { open --raw ($gitdir | path join "HEAD") | str trim } catch { "" })
        if ($head | str starts-with "ref: refs/heads/") {
            $head | str replace "ref: refs/heads/" ""
        } else {
            $head | str substring 0..<7
        }
    }
    $env.TEMOR_PROMPT_COMMAND = {||
        let esc = (ansi escape)
        # A home of "/" (or none) is never shown as "~", as in the other shells.
        let dir = if ($nu.home-dir | is-empty) or ($nu.home-dir == "/") {
            $env.PWD
        } else {
            match (do -i { $env.PWD | path relative-to $nu.home-dir }) {
                null => $env.PWD
                "" => "~"
                $relative => ([~ $relative] | path join)
            }
        }
        let branch = (_temor_branch)
        let branch_part = if ($branch | is-empty) { "" } else { $"  ($esc)[35m($branch)($esc)[0m" }
        let blank = if ($env.TEMOR_SEEN? | is-empty) { "" } else { "\n" }
        $"($blank)($esc)[90m($dir)($esc)[39m($branch_part)\n"
    }
    # This file runs before config.nu, which may set a prompt of its own, so temor's is put back
    # before every prompt.
    $env.config.hooks.pre_prompt = (
        $env.config.hooks.pre_prompt? | default [] | append {||
            $env.PROMPT_COMMAND = $env.TEMOR_PROMPT_COMMAND
            $env.PROMPT_INDICATOR = ""
            $env.PROMPT_COMMAND_RIGHT = {|| "" }
        }
    )
    $env.PROMPT_COMMAND = $env.TEMOR_PROMPT_COMMAND
    $env.PROMPT_INDICATOR = ""
    $env.PROMPT_COMMAND_RIGHT = {|| "" }
    # The blank line is tied to commands, not to drawing: there is one before every prompt once a
    # command has run.
    $env.config.hooks.pre_execution = (
        $env.config.hooks.pre_execution? | default [] | append {|| $env.TEMOR_SEEN = "1" }
    )
}

# Tells temor the shell's exported variables (OSC 7503, see temor-shell's report module) before
# every prompt, but only when they differ from what it last said. Only text values are said: a
# list or a record has no one way to be written. Too large a report is not sent. The report key
# temor gave this shell is taken out of the environment at load (the hook keeps it), so no program
# sees it, and signs each report; a shell without it (one started inside the pane) says nothing.
if ($env.TEMOR_SHELL_INTEGRATION? != "0") {
    let key = ($env.TEMOR_REPORT_KEY? | default "")
    hide-env --ignore-errors TEMOR_REPORT_KEY
    $env.config.hooks.pre_prompt = (
        $env.config.hooks.pre_prompt? | default [] | append {||
          if $key != "" {
            let skip = [_ PWD OLDPWD SHLVL FILE_PWD CURRENT_FILE LAST_EXIT_CODE CMD_DURATION_MS PROMPT_COMMAND PROMPT_COMMAND_RIGHT PROMPT_INDICATOR TEMOR_PROMPT_COMMAND TEMOR_REPORTED]
            let pairs = ($env | transpose name value
                | where {|row| ($row.value | describe) == "string" and not ($row.name in $skip) })
            let raw = ($pairs | each {|row| $"e=($row.name)=($row.value)" } | str join "\n")
            if $raw != ($env.TEMOR_REPORTED? | default "") {
                $env.TEMOR_REPORTED = $raw
                let entries = ($pairs | each {|row|
                    $";e=($row.name | url encode --all)=($row.value | url encode --all)"
                } | str join "")
                let out = $"v2;k=($key)($entries)"
                if ($out | str length) <= 60000 {
                    print -n $"(ansi escape)]7503;($out)(ansi escape)\\"
                }
            }
          }
        }
    )
}

# A program that erases the scrollback (CSI 3 J) to redraw itself takes only its own rows; the
# blocks above it stay. Your own `clear` should take them all, so it says so to temor first
# (not `clear -x`, which keeps the scrollback). It
# clears the way nushell's own does; a `clear` defined in your config.nu, which runs later, wins.
def clear [
    --keep-scrollback (-k) # Clear only the screen, not the scrollback
] {
    if (not $keep_scrollback) and ($env.TEMOR_SHELL_INTEGRATION? != "0") { print -n $"(ansi esc)]7502;clear(char bel)" }
    if $keep_scrollback {
        print -n $"(ansi esc)[H(ansi esc)[2J"
    } else {
        print -n $"(ansi esc)[H(ansi esc)[2J(ansi esc)[3J"
    }
}

# Take temor's folder back out of XDG_DATA_DIRS, so programs started from this shell never see
# it: restore the value temor found, or unset the variable when it was unset.
if ("TEMOR_NU_XDG_DATA_DIRS" in $env) {
    $env.XDG_DATA_DIRS = $env.TEMOR_NU_XDG_DATA_DIRS
    hide-env TEMOR_NU_XDG_DATA_DIRS
} else {
    hide-env -i XDG_DATA_DIRS
}
