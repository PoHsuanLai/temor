# temor shell integration for nushell.
#
# nushell 0.9x and newer can mark prompts and commands itself: osc133 (prompt start, input start,
# command start and command end with its exit status) and osc7 (the working directory). temor
# turns both on here, from a vendor autoload file in its own folder, so your config.nu is never
# edited. nushell reads this file before your config. TEMOR_SHELL_INTEGRATION=0 turns it off.
$env.config.shell_integration.osc133 = true
$env.config.shell_integration.osc7 = true

# It also replaces nushell's stock prompt (the PROMPT_COMMAND that ships with nushell) with a
# clean two-line one: a blank line, the folder and the git branch, then an empty line to type on.
# A prompt you set yourself is never touched. TEMOR_PROMPT=shell keeps nushell's own prompt even
# when it is the stock one; TEMOR_PROMPT=temor (or unset) means "temor's when nushell's is stock".
# Colours are ANSI indices, so the terminal's palette themes them.
if ($env.TEMOR_PROMPT? != "shell") and (try { view source $env.PROMPT_COMMAND | str contains 'path relative-to $nu.home-dir' } catch { false }) {
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
    $env.PROMPT_COMMAND = {||
        let esc = (ansi escape)
        let dir = match (do -i { $env.PWD | path relative-to $nu.home-dir }) {
            null => $env.PWD
            "" => "~"
            $relative => ([~ $relative] | path join)
        }
        let branch = (_temor_branch)
        let branch_part = if ($branch | is-empty) { "" } else { $"  ($esc)[2;35m($branch)($esc)[0m" }
        let blank = if ($env.TEMOR_SEEN? | is-empty) { "" } else { "\n" }
        $"($blank)($esc)[2m($dir)($esc)[22m($branch_part)\n"
    }
    $env.PROMPT_INDICATOR = ""
    $env.PROMPT_COMMAND_RIGHT = {|| "" }
    # The blank line is tied to commands, not to drawing: there is one before every prompt once a
    # command has run.
    $env.config.hooks.pre_execution = (
        $env.config.hooks.pre_execution? | default [] | append {|| $env.TEMOR_SEEN = "1" }
    )
}

# Take temor's folder back out of XDG_DATA_DIRS, so programs started from this shell never see
# it: restore the value temor found, or unset the variable when it was unset.
if ("TEMOR_NU_XDG_DATA_DIRS" in $env) {
    $env.XDG_DATA_DIRS = $env.TEMOR_NU_XDG_DATA_DIRS
    hide-env TEMOR_NU_XDG_DATA_DIRS
} else {
    hide-env -i XDG_DATA_DIRS
}
