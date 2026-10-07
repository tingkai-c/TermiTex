"""Headless PTY integration. Does not claim visual correctness in Ghostty."""
import errno
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import termios
import time

root = Path(__file__).resolve().parents[1]
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 100, 1600, 1020))
child = r'''
import sys,time
sys.stdout.write("\x1b[?1049h\x1b[?2026h\x1b[2J\x1b[HHello \\(x^2\\)\r\n\x1b[?2026l")
sys.stdout.flush()
time.sleep(2)
sys.stdout.write("\x1b[?2026h\x1b[2J\x1b[5;1HHello \\(x^2\\)\x1b[?2026l")
sys.stdout.flush()
time.sleep(1)
sys.stdout.write("\x1b[?1049l")
sys.stdout.flush()
'''
proc = subprocess.Popen([str(root/'target/release/termitex'), '--', sys.executable, '-c', child],
                        stdin=slave, stdout=slave, stderr=slave, close_fds=True)
os.close(slave)
output = bytearray()
probe_sent = False
try:
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        if select.select([master], [], [], .1)[0]:
            try:
                chunk = os.read(master, 65536)
            except OSError as e:
                if e.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            output.extend(chunk)
            if b'\x1b[16t' in output and not probe_sent:
                os.write(master, b'\x1b[6;34;16t')
                # Probe emitted once in this test.
                probe_sent = True
        elif proc.poll() is not None:
            break
    assert proc.wait(timeout=2) == 0
    assert b'Hello' in output
    assert output.count(b'\x1b_Ga=p,') >= 2, 'expected placements before and after redraw'
    assert b'\x1b[5;7H\x1b_Ga=p,' in output, 'image must move to current formula location'
    print('PTY: real worker produced placements before and after screen movement')
finally:
    if proc.poll() is None:
        proc.kill()
        proc.wait()
    os.close(master)
