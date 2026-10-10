# temor's SSH bootstrap (PLAN §9b, Tier A). The local ssh sends this, base64-encoded and run by
# `sh`, with each connection, and nothing is installed on the host. Everything it writes lives in a
# temp directory that is removed when the shell exits. Your dotfiles are never touched.

# Whether this bootstrap may add a terminal description to ~/.terminfo (install or skip), and
# the folder the shell starts in (empty: the login folder).
#@OPTIONS@

temor_base=${TMPDIR:-/tmp}
case $temor_base in
    *:*)
        # XDG_DATA_DIRS separates entries with ':', so the fish shim cannot live under such a path.
        echo "temor: TMPDIR must not contain ':'" >&2
        exit 1 ;;
esac
temor_umask=$(umask)
umask 077

# Removes temp directories of sessions that have ended. A directory is removed only when its name
# has mktemp's exact shape, it is a real directory owned by this user, it carries the marker only
# this bootstrap writes, and its recorded pid is gone.
temor_an='[A-Za-z0-9]'
for temor_old in "$temor_base"/temor.*; do
    case ${temor_old##*/} in
        temor.$temor_an$temor_an$temor_an$temor_an$temor_an$temor_an$temor_an$temor_an) ;;
        *) continue ;;
    esac
    [ -d "$temor_old" ] && [ ! -L "$temor_old" ] && [ -O "$temor_old" ] || continue
    [ -f "$temor_old/temor-bootstrap" ] && [ ! -L "$temor_old/temor-bootstrap" ] || continue
    temor_old_pid=$(cat "$temor_old/pid" 2>/dev/null)
    case $temor_old_pid in
        ''|*[!0-9]*) continue ;;
    esac
    kill -0 "$temor_old_pid" 2>/dev/null || rm -rf "$temor_old"
done

temor_tmp=$(mktemp -d "$temor_base/temor.XXXXXXXX") || exit 1
trap 'rm -rf "$temor_tmp"' EXIT
# The shell we start handles these signals itself. This wrapper must outlive it to clean up.
trap : HUP INT QUIT TERM
printf '%s\n' "$$" > "$temor_tmp/pid"
: > "$temor_tmp/temor-bootstrap"

mkdir -p "$temor_tmp/bash" "$temor_tmp/zsh" "$temor_tmp/fish/fish/vendor_conf.d" \
    "$temor_tmp/nu/nushell/vendor/autoload" || exit 1
#@INTEGRATION@

# Sets temor_enc_out to $1 percent-encoded for a file:// URL, byte by byte. Callers run it in a
# subshell with LC_ALL=C, so a multibyte character is encoded as its bytes.
temor_encode() {
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
# ~/.terminfo is created only once the compiled entry is in hand, and taken back if that fails.
temor_term=xterm-256color
if temor_have_ghostty; then
    temor_term=xterm-ghostty
elif [ "$temor_terminfo" = install ] && command -v tic >/dev/null 2>&1 && [ -n "${HOME:-}" ]; then
#@TERMINFO@
    if tic -x -o "$temor_tmp/terminfo" "$temor_tmp/xterm-ghostty.src" >/dev/null 2>&1 \
        && mkdir -p "$HOME/.terminfo/x" 2>/dev/null \
        && cp "$temor_tmp/terminfo/x/xterm-ghostty" "$HOME/.terminfo/x/xterm-ghostty" 2>/dev/null; then
        temor_term=xterm-ghostty
    else
        rmdir "$HOME/.terminfo/x" "$HOME/.terminfo" 2>/dev/null
    fi
fi
export TERM=$temor_term COLORTERM=truecolor

# A session opened beside another starts in that one's folder, when it is still there.
if [ -n "$temor_start_dir" ]; then
    cd -- "$temor_start_dir" 2>/dev/null || :
fi

# OSC 7 with this host's name, so temor knows where the pane is before the first prompt.
temor_host=$(hostname 2>/dev/null || uname -n 2>/dev/null)
temor_host_enc=$(LC_ALL=C; temor_encode "$temor_host"; printf '%s' "$temor_enc_out")
temor_pwd_enc=$(LC_ALL=C; temor_encode "$PWD"; printf '%s' "$temor_enc_out")
printf '\033]7;file://%s%s\033\\' "$temor_host_enc" "$temor_pwd_enc"

# The login shell, with temor's integration loaded after the person's own startup files.
temor_shell=${SHELL:-}
if [ -z "$temor_shell" ] || [ ! -x "$temor_shell" ]; then
    temor_shell=/bin/sh
fi
# The person's shell starts with the umask they had before the bootstrap.
umask "$temor_umask"
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
    nu|nushell)
        # nushell reads vendor autoload files from XDG_DATA_DIRS; temor.nu takes this entry back
        # out, and puts back the value found here (TEMOR_NU_XDG_DATA_DIRS), once it has run.
        if [ "${XDG_DATA_DIRS+set}" = set ]; then
            TEMOR_NU_XDG_DATA_DIRS=$XDG_DATA_DIRS
            export TEMOR_NU_XDG_DATA_DIRS
        fi
        XDG_DATA_DIRS="$temor_tmp/nu:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}" \
            "$temor_shell" -l ;;
    *)
        "$temor_shell" ;;
esac
temor_status=$?
exit "$temor_status"
