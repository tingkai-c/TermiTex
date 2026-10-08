# Terminal compatibility

TermiTex uses one Kitty graphics backend across these targets. Run this in the terminal you want to use:

```sh
termitex doctor
termitex -- codex -c tui.rendering.math=false
```

`doctor` reports detected graphics transport, synchronized updates, pixel cell dimensions, and terminal identity. It does not launch Codex or a math worker. An affirmative graphics reply confirms transport availability, not complete protocol conformance or smooth scrolling.

| Target | Setup | Verification |
| :--- | :--- | :--- |
| Ghostty · macOS/Linux | Automatic capability probing | Existing macOS visual baseline; shared layout regression tests |
| Kitty · macOS/Linux | Automatic capability probing | Real Linux protocol smoke test in CI; visual checklist below |
| WezTerm · macOS/Linux | Set `config.enable_kitty_graphics = true` in `wezterm.lua` | Shared transport implemented; emulator-specific visual validation pending |
| iTerm2 · macOS | Use a version with Kitty graphics support and check `doctor` | Shared transport implemented; minimum version and visual validation pending |
| Konsole · Linux | Check `doctor` for the installed version | Real Linux protocol smoke test in CI; visual checklist below |

The [latest CI run](https://github.com/tingkai-c/TermiTex/actions/workflows/ci.yml) records tested Linux terminal versions and probe transcripts as `terminal-probe-reports`. Test/build artifacts contain native binaries and license notices for all four OS/architecture combinations. They are development snapshots; the Homebrew tap remains pinned to the published release.

## Controls

```toml
# ~/.config/termitex/config.toml
graphics = "auto" # auto, kitty, or off
```

- `auto`: render after a positive graphics probe. Otherwise pass through source text without starting a renderer.
- `--graphics kitty`: force Kitty transport if you know the terminal supports it but a slow/remote connection or intermediary prevents timely detection.
- `--graphics off`: skip all TermiTex probes, image commands, and text projection.

CLI graphics selection overrides the config file. Terminal identities are not used as a substitute for successful probing. Unknown terminals that implement the protocol can also be detected.

Cell size is queried at startup. If no reply arrives, TermiTex uses PTY pixel dimensions, or finally 16 × 34 px. Resize events use updated PTY dimensions when available. Font-size changes on terminals that do not update PTY pixel dimensions currently require restarting TermiTex. Probe timeout is 500 ms; replies that arrive after negotiation can still reach the child, so use a direct local terminal when collecting baseline reports.

Use a direct terminal session for initial testing. tmux, screen, SSH, and terminal multiplexing introduce separate transport and geometry behavior and are not included in the initial compatibility claim.

## Visual checklist

Use the same application and formulas in each terminal, with both compact and compatibility layouts:

1. Inline math, fractions, integrals, matrices, and display blocks render at the right size and position.
2. Wrapped inline formulas and aligned table columns match the Ghostty baseline.
3. During application scrolling, cached equations move with their text without waiting for typesetting.
4. Scroll away and return; check for missing images, stationary images, duplicates, and stale source text.
5. Resize narrower/wider and change font size; check the reported cell metrics and equation placement.
6. The composer and code blocks remain readable source text. Typing, paste, Ctrl-C, and normal exit still work.
7. Switch between normal and alternate screens, clear the screen, and quit. No TermiTex images should remain over unrelated content.
8. Separately test native terminal scrollback; application-driven redraws and terminal scrollback are different behaviors.

For reports, include `termitex doctor`, the terminal's exact version, OS/architecture, renderer/layout, and a small reproducible transcript. Do not infer visual support from headless tests alone.

## Protocol references

[Kitty graphics](https://sw.kovidgoyal.net/kitty/graphics-protocol/) · [WezTerm tracking issue](https://github.com/wezterm/wezterm/issues/986) · [iTerm2 image documentation](https://iterm2.com/documentation-images.html) · [Konsole implementation](https://github.com/KDE/konsole/blob/master/src/Vt102Emulation.cpp)
