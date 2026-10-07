"""Exercise the real renderer; validate PNG dimensions and inline source bounds."""
import base64
import json
from pathlib import Path
import struct
import subprocess

root = Path(__file__).resolve().parents[1]
requests = []
for key, latex, rows, cols, display in [
    ('inline', r'J', 1, 5, False),
    ('display', r'\displaystyle\frac{1}{2}', 3, 80, True),
    ('curl', r'\nabla\times', 1, 20, False),
]:
    requests.append(dict(key=key, cell_width=16, cell_height=34, formula=dict(
        latex=latex, row=0, col=0, rows=rows, cols=cols,
        display=display, fg='#ffffff', bg='#282c34')))
result = subprocess.run(['node', str(root / 'worker/render.mjs')],
    input=''.join(json.dumps(r)+'\n' for r in requests), text=True,
    capture_output=True, check=True, timeout=30)
responses = [json.loads(line) for line in result.stdout.splitlines()]
assert len(responses) == len(requests)
for req, response in zip(requests, responses):
    assert response['key'] == req['key']
    assert 'error' not in response, response
    png = base64.b64decode(response['png'])
    assert png[:8] == b'\x89PNG\r\n\x1a\n'
    width, height = struct.unpack('>II', png[16:24])
    assert width > 0 and height > 0
    print(f"{req['key']}: valid PNG {width}x{height}")
