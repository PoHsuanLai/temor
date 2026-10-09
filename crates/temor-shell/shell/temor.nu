# temor shell integration for nushell.
#
# nushell 0.9x and newer can mark prompts and commands itself: osc133 (prompt start, input start,
# command start and command end with its exit status) and osc7 (the working directory). temor
# turns both on here, from a vendor autoload file in its own folder, so your config.nu is never
# edited. nushell reads this file before your config. TEMOR_SHELL_INTEGRATION=0 turns it off.
$env.config.shell_integration.osc133 = true
$env.config.shell_integration.osc7 = true

# Take temor's folder back out of XDG_DATA_DIRS, so programs started from this shell never see
# it: restore the value temor found, or unset the variable when it was unset.
if ("TEMOR_NU_XDG_DATA_DIRS" in $env) {
    $env.XDG_DATA_DIRS = $env.TEMOR_NU_XDG_DATA_DIRS
    hide-env TEMOR_NU_XDG_DATA_DIRS
} else {
    hide-env -i XDG_DATA_DIRS
}
