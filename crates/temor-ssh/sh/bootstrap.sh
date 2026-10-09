# temor's SSH bootstrap (PLAN §9b, Tier A). The local ssh sends this as `sh -c` with each
# connection, and nothing is installed on the host. Everything it writes lives in a temp directory
# that is removed when the shell exits. Your dotfiles are never touched.

umask 077
temor_tmp=$(mktemp -d "${TMPDIR:-/tmp}/temor.XXXXXXXX") || exit 1
trap 'rm -rf "$temor_tmp"' EXIT
# The shell we start handles these signals itself. This wrapper must outlive it to clean up.
trap : HUP INT QUIT TERM

mkdir -p "$temor_tmp/bash" "$temor_tmp/zsh" "$temor_tmp/fish/fish/vendor_conf.d" || exit 1
#@INTEGRATION@

# Sets temor_enc_out to $1 percent-encoded for a file:// URL, byte by byte.
temor_encode() {
    LC_ALL=C
    temor_enc_in=$1
    temor_enc_out=
    while [ -n "$temor_enc_in" ]; do
        temor_enc_ch=${temor_enc_in%"${temor_enc_in#?}"}
        temor_enc_in=${temor_enc_in#?}
        case $temor_enc_ch in
            [a-zA-Z0-9/._~-]) temor_enc_out=$temor_enc_out$temor_enc_ch ;;
            *) temor_enc_out=$temor_enc_out$(printf '%%%02X' "'$temor_enc_ch") ;;
        esac
    done
}

# Succeeds when xterm-ghostty is already on this host.
temor_have_ghostty() {
    if command -v infocmp >/dev/null 2>&1; then
        infocmp xterm-ghostty >/dev/null 2>&1
        return
    fi
    for temor_dir in "${HOME:-/nonexistent}/.terminfo" /etc/terminfo /lib/terminfo \
        /usr/share/terminfo /usr/lib/terminfo /usr/local/share/terminfo; do
        [ -f "$temor_dir/x/xterm-ghostty" ] && return 0
    done
    return 1
}

# Ghostty's terminfo when the host has it or takes it into ~/.terminfo, else xterm-256color.
temor_term=xterm-256color
if temor_have_ghostty; then
    temor_term=xterm-ghostty
elif command -v tic >/dev/null 2>&1 && [ -n "${HOME:-}" ] && mkdir -p "$HOME/.terminfo" 2>/dev/null; then
#@TERMINFO@
    if tic -x -o "$HOME/.terminfo" "$temor_tmp/xterm-ghostty.src" >/dev/null 2>&1; then
        temor_term=xterm-ghostty
    fi
fi
export TERM=$temor_term COLORTERM=truecolor

# OSC 7 with this host's name, so temor knows where the pane is before the first prompt.
temor_host=$(hostname 2>/dev/null || uname -n 2>/dev/null)
temor_encode "$temor_host"
temor_host_enc=$temor_enc_out
temor_encode "$PWD"
printf '\033]7;file://%s%s\033\\' "$temor_host_enc" "$temor_enc_out"

# The login shell, with temor's integration loaded after the person's own startup files.
temor_shell=${SHELL:-}
if [ -z "$temor_shell" ] || [ ! -x "$temor_shell" ]; then
    temor_shell=/bin/sh
fi
case ${temor_shell##*/} in
    bash)
        TEMOR_BASH_LOGIN=1 "$temor_shell" --rcfile "$temor_tmp/bash/temor-bashrc" -i ;;
    zsh)
        if [ "${ZDOTDIR+set}" = set ]; then
            TEMOR_ZSH_ZDOTDIR=$ZDOTDIR
            export TEMOR_ZSH_ZDOTDIR
        fi
        ZDOTDIR="$temor_tmp/zsh" "$temor_shell" -l -i ;;
    fish)
        if [ "${XDG_DATA_DIRS+set}" = set ]; then
            TEMOR_FISH_XDG_DATA_DIRS=$XDG_DATA_DIRS
            export TEMOR_FISH_XDG_DATA_DIRS
        fi
        XDG_DATA_DIRS="$temor_tmp/fish:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}" \
            "$temor_shell" -l -i ;;
    *)
        "$temor_shell" ;;
esac
temor_status=$?
exit "$temor_status"
