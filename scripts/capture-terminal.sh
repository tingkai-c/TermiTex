#!/usr/bin/env bash
# Run inside an isolated Xvfb display. Captures actual terminal pixels, not PNGs
# produced by the math worker. Requires kitty/konsole, ImageMagick and xdotool.
set -euxo pipefail
terminal=${1:?terminal}
layout=${2:?layout}
root=$(pwd)
out="$root/terminal-reports/$terminal-$layout"
mkdir -p "$out"
control=$(mktemp -d)
cleanup() {
  status=$?
  if [[ $status -ne 0 ]]; then import -window root "$out/failure.png" || true; fi
  touch "$control/exit"
  if [[ -n ${terminal_pid:-} ]]; then kill "$terminal_pid" 2>/dev/null || true; fi
  rm -rf "$control"
}
trap cleanup EXIT
command="$root/target/release/termitex --layout $layout -- $root/target/debug/examples/terminal_fixture $control"
case "$terminal" in
  kitty)
    kitty --config NONE -o initial_window_width=1100 -o initial_window_height=850 -o font_size=16 -o background='#282c34' /usr/bin/script -q -e -f -c "$command" "$out/transcript.txt" &
    ;;
  konsole)
    konsole --separate --hide-menubar --hide-tabbar -p 'Font=DejaVu Sans Mono,16,-1,5,50,0,0,0,0,0' -e /usr/bin/script -q -e -f -c "$command" "$out/transcript.txt" &
    ;;
  wezterm)
    wezterm --skip-config --config enable_kitty_graphics=true --config enable_wayland=false --config font_size=16 start --always-new-process -- /usr/bin/script -q -e -f -c "$command" "$out/transcript.txt" &
    ;;
  *) exit 2 ;;
esac
terminal_pid=$!
for _ in $(seq 1 100); do
  [[ -f "$control/ready" ]] && break
  sleep 0.1
done
test -f "$control/ready"
window=
for _ in $(seq 1 100); do
  window=$(xdotool search --onlyvisible --class 'kitty|konsole|org.wezfurlong.wezterm' | head -1 || true)
  [[ -n "$window" ]] && break
  sleep 0.1
done
test -n "$window"
xdotool windowsize "$window" 1100 850
# Wait for real placements to reach the emulator, then allow the GUI to paint.
for _ in $(seq 1 100); do
  [[ $(grep -ao 'a=p,' "$out/transcript.txt" | wc -l) -ge 6 ]] && break
  sleep 0.1
done
sleep 1
import -window root "$out/initial.png"
touch "$control/scroll"
for _ in $(seq 1 100); do
  [[ -f "$control/scrolled" ]] && break
  sleep 0.1
done
sleep 1
import -window root "$out/scrolled.png"
xdotool windowsize "$window" 880 740
sleep 1
import -window root "$out/resized.png"
touch "$control/exit"
wait "$terminal_pid"
# Capture even failures so absent/incorrect images remain inspectable.
grep -aq 'a=p,' "$out/transcript.txt"
