"""Renders ShareXrs's region overlay for each shot annotate-tools.ps1 took
of ShareX (the same steps, through `sharexrs --render region-* --steps`)
and compares them with scripts/compare.py.

usage: python scripts/compare-annotate.py <sharex shots dir> <out dir> [tool ...]
"""
import os
import re
import subprocess
import sys

ORDER = ['Select', 'Rectangle', 'Ellipse', 'Line', 'Arrow', 'Freehand', 'Text', 'SpeechBalloon', 'Step',
         'Image', 'Clone', 'Emoji', 'Cursor', 'Highlight', 'SmartEraser', 'Blur', 'Pixelate', 'Magnify', 'Spotlight']
DRAWN = ['Rectangle', 'Ellipse', 'Line', 'Arrow', 'Freehand', 'SpeechBalloon', 'Highlight', 'Blur', 'Pixelate',
         'Magnify', 'Spotlight']


def shots(tool):
    """(name, steps) per shot, as annotate-tools.ps1 takes them."""
    steps = [f'tool:{tool}', 'move:600,600']
    out = [('tool', list(steps))]
    if tool in DRAWN:
        steps += ['down:200,200', 'to:450,400']
        out.append(('draw', list(steps)))
        steps += ['up', 'move:600,600']
        out.append(('selected', list(steps)))
    elif tool == 'Step':
        steps += ['down:300,300']
        out.append(('draw', list(steps)))
        steps += ['up', 'move:600,600']
        out.append(('selected', list(steps)))
    elif tool == 'Text':
        steps += ['click:200,200', 'type:Hello', 'move:600,600']
        out.append(('draw', list(steps)))
        steps += ['key:Enter']
        out.append(('selected', list(steps)))
    return out


def main():
    ref, out = sys.argv[1], sys.argv[2]
    tools = sys.argv[3:] or ORDER
    exe = os.path.join(os.path.dirname(__file__), '..', 'target', 'debug', 'sharexrs.exe')
    os.makedirs(out, exist_ok=True)
    for tool in tools:
        for name, steps in shots(tool):
            base = f'{tool.lower()}-{name}'
            sharex = os.path.join(ref, base + '.png')
            if not os.path.exists(sharex):
                continue
            mine = os.path.join(out, base + '.png')
            subprocess.run([exe, '--render', 'region-annotate', '--steps', ';'.join(steps), '--pointer', '600,600',
                            '--out', mine], check=True, capture_output=True)
            r = subprocess.run([sys.executable, os.path.join(os.path.dirname(__file__), 'compare.py'), sharex, mine,
                                os.path.join(out, 'cmp-' + base)], capture_output=True, text=True)
            m = re.search(r'\((\d+\.\d+)%\)', r.stdout)
            print(f'{base:28} {m.group(1) if m else "?"}%')


main()
