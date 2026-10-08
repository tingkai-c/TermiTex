# Installing TermiTex

[Back to the README](../README.md)

## Standalone installer (macOS and Linux)

The installer supports x86_64 and ARM64 release binaries. It needs `curl`, `tar`, and either `sha256sum` or `shasum`; it does not require Rust, Node, or root access. Use a [supported terminal](terminals.md) and install your coding CLI separately.

Download the script, inspect it if desired, and run it:

```sh
curl -fsSL https://raw.githubusercontent.com/tingkai-c/TermiTex/main/install.sh -o /tmp/termitex-install.sh
less /tmp/termitex-install.sh
sh /tmp/termitex-install.sh
```

Piping is also supported; interactive questions read from the terminal rather than from the downloaded script:

```sh
curl -fsSL https://raw.githubusercontent.com/tingkai-c/TermiTex/main/install.sh | sh
```

The script resolves the latest stable GitHub release, selects the OS/CPU archive, verifies its SHA-256 checksum, and runs `--version` before installing. It stages the replacement and renames it into place, so a failed download or incompatible binary leaves the existing executable intact. Checksums detect corrupt or mismatched downloads; they are fetched from the same release as the binaries.

Linux binaries currently come from Ubuntu 24.04 builds. Older glibc systems and musl-based distributions such as Alpine may need a source build. The installer reports loader errors instead of installing a binary that cannot run. Windows is not supported by this script.

By default, the binary goes in `~/.local/bin`. The installation receipt and license notices go in `${XDG_DATA_HOME:-~/.local/share}/termitex/install`. The receipt lets subsequent runs recognize installations owned by this installer. Homebrew installations, manually installed binaries, and symlinked development builds are not overwritten, even with `--force`.

## Updates and reinstalls

Run the script again to update. It prints the installed and target versions. The same version is a no-op unless `--force` is supplied. A missing managed binary is restored automatically; a damaged executable can be repaired with `--force`.

```sh
sh /tmp/termitex-install.sh
sh /tmp/termitex-install.sh --force
sh /tmp/termitex-install.sh --version v0.2.3
```

An explicitly requested version can downgrade an installation. Checking for the latest release never downgrades a newer installed version. Updates, repairs, and forced reinstalls do not prompt for or change shell integration.

Use an absolute custom directory on the first install:

```sh
sh /tmp/termitex-install.sh --bin-dir "$HOME/tools/bin"
```

The installer remembers this location. It manages one destination per data directory. If another installation is already on PATH, the default install stops with instructions; an explicit `--bin-dir` allows a separate copy and reports any PATH precedence conflict.

## Optional shell integration

On the first interactive install, the installer identifies the shell using `$SHELL`, shows the target configuration path and guarded source line, and offers:

1. Codex
2. Claude Code
3. Both
4. PATH only
5. Skip (the default)

Accepted setup adds the install directory to PATH and, if selected, wraps `codex` or `claude` with TermiTex. It does not install those CLIs. Existing aliases/functions take precedence. Aliases are only activated in interactive shells; subprocesses still find the original CLI executable.

The generated integration lives in `${XDG_CONFIG_HOME:-~/.config}/termitex/shell.bash`, `shell.zsh`, or `shell.fish`. The user config receives one guarded source line per file. For example:

```sh
[ ! -f '/home/user/.config/termitex/shell.zsh' ] || . '/home/user/.config/termitex/shell.zsh' # TermiTex
```

Shell configuration files:

- **Zsh:** `${ZDOTDIR:-$HOME}/.zshrc`.
- **Bash:** `~/.bashrc` and the first existing login profile (`.bash_profile`, `.bash_login`, `.profile`), or a new `.bash_profile` if none exists. Both source the same idempotent integration file.
- **Fish:** `${XDG_CONFIG_HOME:-$HOME/.config}/fish/config.fish`.

Existing config files are backed up before appending the source line. Repeating setup does not duplicate that line. The installer does not execute the user's config. Unsupported shells receive manual guidance.

Open a new shell to activate setup. To bypass integration for one invocation:

```sh
command codex
command claude
```

Run setup later without downloading or reinstalling the binary:

```sh
sh /tmp/termitex-install.sh --setup
sh /tmp/termitex-install.sh --setup --shell zsh --integrate both
```

For automation, no terminal or `CI` set means no prompts and no shell modifications. Explicit `--integrate codex|claude|both|path` authorizes setup on a first install (or with `--setup`). `--no-modify-shell` always skips it. `--integrate none` skips setup; it does not remove existing integration.

```sh
sh /tmp/termitex-install.sh --no-modify-shell
```

If you skip setup, add the installation directory to PATH yourself or invoke the absolute executable path printed by the installer.

## Removing integration or uninstalling

Delete the generated `shell.bash`, `shell.zsh`, or `shell.fish` file to disable that shell's integration, then open a new shell. The guarded source line can remain without causing an error, or you can remove its `# TermiTex` line from the shell configuration. The generated file also skips setup if the installed executable has been removed. Existing open shells retain their aliases until restarted.

To uninstall the default standalone installation, remove `~/.local/bin/termitex` and the install receipt/license directory at `${XDG_DATA_HOME:-~/.local/share}/termitex/install`. Remove the generated integration files as described above. For a custom destination, the first line of the receipt records the binary directory. Other TermiTex configuration is left to you to keep or remove.

## Other installation methods

- **Homebrew (macOS):** `brew install tingkai-c/tap/termitex`; update with `brew update && brew upgrade termitex`. Continue using Homebrew to manage this installation.
- **Manual binary:** download the matching archive and `SHA256SUMS` from [GitHub Releases](https://github.com/tingkai-c/TermiTex/releases/latest), verify the checksum, extract it, and place `termitex/termitex` on PATH. Keep the bundled license notices.
- **Source:** follow the Rust build instructions in the [README](../README.md#quick-start).
