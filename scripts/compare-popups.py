"""Renders ShareXrs's region overlay with each option popup open, as
scripts/annotate-popups.ps1 opened ShareX's (the same clicks, through
`sharexrs --render region-annotate --steps`), and compares them with
scripts/compare.py.

usage: python scripts/compare-popups.py <sharex shots dir> <out dir> [dark|light] [name ...]
"""
import os
import re
import subprocess
import sys

# name: (tool index, verb, option x), as annotate-popups.ps1.
SHOTS = {
    'rect-bordercolor': (1, 'click', 355),
    'rect-fillcolor': (1, 'click', 420),
    'rect-thickness': (1, 'click', 485),
    'rect-borderstyle': (1, 'click', 578),
    'rect-cornerradius': (1, 'click', 670),
    'rect-shadow': (1, 'rclick', 726),
    'arrow-arrowstyle': (4, 'click', 550),
    'text-textcolor': (6, 'click', 307),
    'text-fontfamily': (6, 'click', 500),
    'text-alignment': (6, 'click', 622),
    'text-fontsize': (6, 'click', 687),
    'step-steptype': (8, 'click', 640),
    'step-startnumber': (8, 'click', 725),
    'cursor-cursortype': (12, 'click', 535),
}


def main():
    ref, out = sys.argv[1], sys.argv[2]
    theme = sys.argv[3] if len(sys.argv) > 3 else 'dark'
    names = sys.argv[4:] or list(SHOTS)
    exe = os.path.join(os.path.dirname(__file__), '..', 'target', 'debug', 'sharexrs.exe')
    os.makedirs(out, exist_ok=True)
    for name in names:
        tool, verb, x = SHOTS[name]
        sharex = os.path.join(ref, f'{name}-{theme}.png')
        if not os.path.exists(sharex):
            continue
        steps = f'click:{150 + 44 * tool},25;move:600,600;{verb}:{x},70'
        mine = os.path.join(out, f'{name}-{theme}.png')
        args = [exe, '--render', 'region-annotate', '--steps', steps, '--pointer', f'{x},70',
                '--work-area', '0,0,1024,720', '--out', mine]
        if theme == 'light':
            args += ['--theme', 'light']
        subprocess.run(args, check=True, capture_output=True)
        r = subprocess.run([sys.executable, os.path.join(os.path.dirname(__file__), 'compare.py'), sharex, mine,
                            os.path.join(out, f'cmp-{name}-{theme}')], capture_output=True, text=True)
        m = re.search(r'\((\d+\.\d+)%\)', r.stdout)
        print(f'{name:22} {theme:5} {m.group(1) if m else "?"}%')


main()
