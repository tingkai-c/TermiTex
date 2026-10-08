#!/bin/sh
# TermiTex standalone installer. Run with sh; no root privileges required.
set -eu

say() { printf '%s\n' "$*"; }
die() { say "Error: $*" >&2; exit 1; }
usage() {
    cat <<'HELP'
Usage: sh install.sh [options]
  --version VERSION       Install a specific release (for example, v0.2.3)
  --bin-dir DIRECTORY     Install directory (default: ~/.local/bin)
  --force                 Reinstall an installer-managed copy, even if current
  --no-modify-shell       Skip shell setup and all prompts
  --integrate CHOICE      First-install setup: codex, claude, both, path, or none
  --shell SHELL           Shell to configure: bash, zsh, or fish (default: $SHELL)
  --setup                 Configure an existing installer-managed installation
  -h, --help              Show this help

Rerun to update. Updates and reinstalls do not modify shell configuration.
Use --setup to change integration later. Noninteractive runs skip setup unless
--integrate is supplied. Existing package-manager/manual installs are not replaced.
HELP
}
valid_version() { printf '%s\n' "$1" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'; }
quote() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}
fetch() { curl --proto '=https' --proto-redir '=https' --fail --silent --show-error --location --retry 3 "$1" -o "$2"; }

main() {
    requested=latest
    bin_dir=
    explicit_dir=false
    force=false
    setup=false
    integration=ask
    no_shell=false
    shell_name=${SHELL:-}
    shell_name=${shell_name##*/}
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --version|--bin-dir|--integrate|--shell)
                [ "$#" -ge 2 ] && [ -n "$2" ] || die "$1 requires a value."
                case "$1" in
                    --version) requested=${2#v} ;;
                    --bin-dir) bin_dir=$2; explicit_dir=true ;;
                    --integrate) integration=$2 ;;
                    --shell) shell_name=$2 ;;
                esac
                shift 2 ;;
            --force) force=true; shift ;;
            --setup) setup=true; shift ;;
            --no-modify-shell) no_shell=true; shift ;;
            -h|--help) usage; return ;;
            *) die "Unknown option: $1 (see --help)." ;;
        esac
    done
    case "$integration" in ask|codex|claude|both|path|none) ;; *) die "Invalid integration choice: $integration" ;; esac
    if [ "$requested" != latest ]; then valid_version "$requested" || die "Expected a stable version such as v0.2.3."; fi
    [ -n "${HOME:-}" ] || die 'HOME must be set.'
    state_dir=${XDG_DATA_HOME:-$HOME/.local/share}/termitex/install
    config_dir=${XDG_CONFIG_HOME:-$HOME/.config}/termitex
    receipt=$state_dir/receipt
    managed=false
    installed_version=
    if [ -f "$receipt" ]; then
        managed=true
        recorded_dir=$(sed -n '1p' "$receipt")
        installed_version=$(sed -n '2p' "$receipt")
        valid_version "$installed_version" || die "Invalid install receipt: $receipt"
        if [ -z "$bin_dir" ]; then bin_dir=$recorded_dir; fi
        [ "$bin_dir" = "$recorded_dir" ] || die "An installation is managed in $recorded_dir. Reuse that directory."
    fi
    bin_dir=${bin_dir:-$HOME/.local/bin}
    case "$bin_dir" in /*) ;; *) die '--bin-dir must be an absolute path.' ;; esac
    # Paths become shell configuration and receipt lines; newlines are not supported.
    case "$bin_dir$config_dir$state_dir" in *'
'*) die 'Installation paths cannot contain newlines.' ;; esac
    exe=$bin_dir/termitex
    [ ! -L "$exe" ] || die "$exe is a symlink. Use its existing installation method or choose another --bin-dir."
    if [ -e "$exe" ]; then
        [ -f "$exe" ] || die "$exe is not a regular file."
        $managed || die "$exe was not installed by this installer. Use its existing installation method or choose another --bin-dir."
    fi
    if ! $managed && ! $explicit_dir; then
        existing=$(command -v termitex 2>/dev/null || true)
        [ -z "$existing" ] || die "Found an existing installation at $existing. Update it with its original method, or use --bin-dir for a separate installation."
    fi
    if $setup; then
        $managed && [ -x "$exe" ] || die '--setup requires an existing installer-managed installation.'
        $no_shell || setup_shell
        return
    fi
    for tool in curl tar mktemp awk sed grep; do command -v "$tool" >/dev/null 2>&1 || die "$tool is required."; done
    command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || die 'sha256sum or shasum is required.'
    case "$(uname -s)" in Darwin) platform=macos ;; Linux) platform=linux ;; *) die 'Supported operating systems: macOS and Linux.' ;; esac
    case "$(uname -m)" in x86_64|amd64) arch=x86_64 ;; arm64|aarch64) arch=aarch64 ;; *) die 'Supported architectures: x86_64 and ARM64.' ;; esac
    releases=https://github.com/tingkai-c/TermiTex/releases
    version=$requested
    if [ "$version" = latest ]; then
        latest_url=$(curl --proto '=https' --proto-redir '=https' --fail --silent --show-error --location --retry 3 -o /dev/null -w '%{url_effective}' "$releases/latest")
        case "$latest_url" in "$releases/tag/v"*) version=${latest_url##*/v} ;; *) die 'Could not resolve the latest stable release.' ;; esac
    fi
    valid_version "$version" || die "Unexpected release version: $version"
    action=install
    if $managed; then
        action=update
        if [ "$installed_version" = "$version" ]; then
            if [ -x "$exe" ] && ! $force; then
                actual=$("$exe" --version 2>/dev/null || true)
                [ "$actual" = "termitex $version" ] || die 'The installed binary is damaged or changed. Rerun with --force to repair it.'
                say "TermiTex $version is already up to date. Use --force to reinstall."
                return
            fi
            action=reinstall
        elif [ "$requested" = latest ] && awk -v old="$installed_version" -v new="$version" 'BEGIN {split(old,a,".");split(new,b,".");for(i=1;i<=3;i++){if(a[i]+0>b[i]+0)exit 0;if(a[i]+0<b[i]+0)exit 1}exit 1}'; then
            say "Installed $installed_version is newer than latest $version; keeping it. Use --version to select an older release explicitly."
            return
        fi
    fi
    say "TermiTex: $action ${installed_version:+$installed_version -> }$version ($platform/$arch)"
    say "Destination: $exe"
    tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/termitex-install.XXXXXXXX")
    stage=
    trap 'rm -rf "$tmp_dir"; if [ -n "$stage" ]; then rm -f "$stage"; fi' 0
    trap 'exit 130' INT
    trap 'exit 143' TERM
    asset=termitex-$platform-$arch.tar.gz
    fetch "$releases/download/v$version/$asset" "$tmp_dir/$asset"
    fetch "$releases/download/v$version/SHA256SUMS" "$tmp_dir/SHA256SUMS"
    expected=$(awk -v name="$asset" '$2 == name { print $1; count++ } END { if (count != 1) exit 1 }' "$tmp_dir/SHA256SUMS") || die 'Missing or duplicate release checksum.'
    [ "${#expected}" -eq 64 ] || die 'Invalid release checksum.'
    case "$expected" in *[!0-9a-f]*) die 'Invalid release checksum.' ;; esac
    [ "$(sha256 "$tmp_dir/$asset")" = "$expected" ] || die 'Checksum mismatch; existing installation was not changed.'
    # Extract only the binary and license files into private staging, never over
    # a running installation. The release archive is trusted after verification.
    tar -xzf "$tmp_dir/$asset" -C "$tmp_dir" termitex/termitex termitex/LICENSE termitex/licenses
    candidate=$tmp_dir/termitex/termitex
    [ -f "$candidate" ] && [ ! -L "$candidate" ] || die 'Release is missing its binary.'
    chmod 755 "$candidate"
    actual=$("$candidate" --version 2>"$tmp_dir/compatibility-error") || {
        cat "$tmp_dir/compatibility-error" >&2
        die 'This binary cannot run on your system. Linux builds require a compatible glibc system; build from source if necessary. Existing installation was not changed.'
    }
    [ "$actual" = "termitex $version" ] || die 'Downloaded binary reports the wrong version.'
    mkdir -p "$bin_dir" "$state_dir"
    cp "$tmp_dir/termitex/LICENSE" "$state_dir/LICENSE"
    mkdir -p "$state_dir/licenses"
    cp -R "$tmp_dir/termitex/licenses/." "$state_dir/licenses/"
    stage=$(mktemp "$bin_dir/.termitex.XXXXXXXX")
    cp "$candidate" "$stage"
    chmod 755 "$stage"
    mv -f "$stage" "$exe"
    stage=
    printf '%s\n%s\n' "$bin_dir" "$version" > "$state_dir/receipt.new"
    mv -f "$state_dir/receipt.new" "$receipt"
    say "Installed TermiTex $version."
    if ! $managed && ! $no_shell; then setup_shell; fi
    case ":${PATH:-}:" in *":$bin_dir:"*) ;; *) say "Add $bin_dir to PATH, or run: $(quote "$exe") codex" ;; esac
    other=$(command -v termitex 2>/dev/null || true)
    if [ -n "$other" ] && [ "$other" != "$exe" ]; then say "Your current PATH selects $other. Open a new shell after setup, or invoke $exe directly."; fi
}

setup_shell() {
    [ "$integration" != none ] || return 0
    case "$shell_name" in
        bash) rc=$HOME/.bashrc ;;
        zsh) rc=${ZDOTDIR:-$HOME}/.zshrc ;;
        fish) rc=${XDG_CONFIG_HOME:-$HOME/.config}/fish/config.fish ;;
        *) say 'Automatic setup supports Bash, Zsh, and Fish. Use termitex codex or termitex claude directly.'; return 0 ;;
    esac
    login_rc=
    if [ "$shell_name" = bash ]; then
        # Bash login shells read only the first existing profile in this list.
        login_rc=$HOME/.bash_profile
        for profile in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
            if [ -f "$profile" ]; then login_rc=$profile; break; fi
        done
    fi
    integration_file=$config_dir/shell.$shell_name
    if [ "$shell_name" = fish ]; then
        source_line="test ! -f $(quote "$integration_file"); or source $(quote "$integration_file") # TermiTex"
    else
        source_line="[ ! -f $(quote "$integration_file") ] || . $(quote "$integration_file") # TermiTex"
    fi
    if [ "$integration" = ask ]; then
        # Do not consume the script itself when invoked through curl | sh.
        if [ -n "${CI:-}" ] || ! [ -t 1 ] || ! ( : </dev/tty ) 2>/dev/null; then
            say 'Shell setup skipped (noninteractive). Run this installer with --setup later.'
            return 0
        fi
        say "Shell setup adds $bin_dir to PATH and this line to $rc:"
        say "  $source_line"
        if [ -n "$login_rc" ]; then say "The same line is added to $login_rc for Bash login shells."; fi
        say 'Automatically wrap coding CLI commands with TermiTex?'
        say '  1. Codex   2. Claude Code   3. Both   4. PATH only   5. Skip [default]'
        printf 'Choose [1-5]: '
        answer=
        read -r answer </dev/tty || true
        case "$answer" in 1) integration=codex ;; 2) integration=claude ;; 3) integration=both ;; 4) integration=path ;; *) return 0 ;; esac
    fi
    mkdir -p "$config_dir"
    # Own only this generated file. Never execute or rewrite user shell config.
    if [ -e "$integration_file" ] && ! grep -qFx '# Generated by the TermiTex installer.' "$integration_file"; then
        say "Shell setup skipped: $integration_file already exists and is not managed by TermiTex."
        return 0
    fi
    shell_tmp=$(mktemp "$config_dir/.shell.XXXXXXXX")
    {
        say '# Generated by the TermiTex installer.'
        say '# Remove this file to disable setup; the guarded source line remains harmless.'
        if [ "$shell_name" = fish ]; then
            say "if not test -x $(quote "$exe")"
            say '    return 0'
            say 'end'
            say "if not contains -- $(quote "$bin_dir") \$PATH"
            say "    set -gx PATH $(quote "$bin_dir") \$PATH"
            say 'end'
            say 'if status is-interactive'
        else
            say "[ -x $(quote "$exe") ] || return 0"
            say "case \":\$PATH:\" in *:$(quote "$bin_dir"):*) ;; *) export PATH=$(quote "$bin_dir"):\"\$PATH\" ;; esac"
            say 'case $- in *i*)'
        fi
        for cli in codex claude; do
            if [ "$integration" != both ] && [ "$integration" != "$cli" ]; then continue; fi
            if [ "$shell_name" = fish ]; then
                say "    if not functions -q $cli"
                say "        function $cli --wraps $cli"
                say "            command $(quote "$exe") $cli \$argv"
                say '        end'
                say '    end'
            else
                say "    if ! alias $cli >/dev/null 2>&1 && ! typeset -f $cli >/dev/null 2>&1; then"
                say "        alias $cli=$(quote "$(quote "$exe") $cli")"
                say '    fi'
            fi
        done
        if [ "$shell_name" = fish ]; then say end; else say ';; esac'; fi
    } > "$shell_tmp"
    chmod 644 "$shell_tmp"
    mv -f "$shell_tmp" "$integration_file"
    append_source "$rc"
    if [ -n "$login_rc" ]; then append_source "$login_rc"; fi
    say "Shell integration: $integration ($integration_file). Existing aliases and functions take precedence."
    say "Open a new shell, or run: source $(quote "$rc")"
    say 'Use command codex or command claude to bypass the aliases.'
}
append_source() {
    target_rc=$1
    mkdir -p "$(dirname "$target_rc")"
    if [ -f "$target_rc" ] && grep -qFx "$source_line" "$target_rc"; then return; fi
    if [ -f "$target_rc" ]; then
        backup=$(mktemp "$target_rc.termitex-backup.XXXXXXXX")
        cp -p "$target_rc" "$backup"
        say "Backed up $target_rc to $backup"
    fi
    printf '\n%s\n' "$source_line" >> "$target_rc"
    say "Added guarded source line to $target_rc"
}

# Keep the entry point last: a truncated piped download cannot start installation.
main "$@"
