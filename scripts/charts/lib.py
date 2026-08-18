"""Minimal SVG chart helpers, following the dataviz skill's mark specs."""
import math

# Reference palette, light mode (references/palette.md). Validated:
# node scripts/validate_palette.js "#2a78d6,#eb6834" --mode light -> ALL PASS
SURFACE   = "#fcfcfb"
INK       = "#0b0b0b"
INK2      = "#52514e"
MUTED     = "#8a8981"
GRID      = "#e6e5e0"
S1        = "#2a78d6"   # slot 1 blue
S2        = "#eb6834"   # slot 2 orange
S3        = "#1baf7a"   # slot 3 aqua
FONT      = "Helvetica, Arial, sans-serif"

def esc(s):
    return (str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))

class Svg:
    def __init__(self, w, h):
        self.w, self.h, self.parts = w, h, []
        self.parts.append(f'<rect width="{w}" height="{h}" fill="{SURFACE}"/>')
    def add(self, s): self.parts.append(s); return self
    def text(self, x, y, s, size=13, fill=INK2, weight="400", anchor="start", family=FONT):
        self.add(f'<text x="{x:.1f}" y="{y:.1f}" font-family="{family}" font-size="{size}" '
                 f'font-weight="{weight}" fill="{fill}" text-anchor="{anchor}">{esc(s)}</text>')
        return self
    def line(self, x1, y1, x2, y2, stroke=GRID, width=1, dash=None):
        d = f' stroke-dasharray="{dash}"' if dash else ""
        self.add(f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" '
                 f'stroke="{stroke}" stroke-width="{width}"{d}/>')
        return self
    def rect(self, x, y, w, h, fill, r=0):
        if w <= 0 or h <= 0: return self
        self.add(f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" '
                 f'rx="{r}" fill="{fill}"/>')
        return self
    def path(self, d, stroke, width=2, fill="none", cap="round", join="round"):
        self.add(f'<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{width}" '
                 f'stroke-linecap="{cap}" stroke-linejoin="{join}"/>')
        return self
    def dot(self, x, y, r=4.5, fill=S1, ring=SURFACE):
        # 2px surface ring on overlapping marks (marks-and-anatomy).
        self.add(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r+2:.1f}" fill="{ring}"/>')
        self.add(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r:.1f}" fill="{fill}"/>')
        return self
    def save(self, path):
        head = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w}" height="{self.h}" '
                f'viewBox="0 0 {self.w} {self.h}">')
        open(path, "w").write(head + "".join(self.parts) + "</svg>")

def nice_ticks(lo, hi, n=5):
    span = hi - lo
    if span <= 0: return [lo]
    raw = span / n
    mag = 10 ** math.floor(math.log10(raw))
    for m in (1, 2, 2.5, 5, 10):
        if raw <= m * mag:
            step = m * mag; break
    start = math.ceil(lo / step) * step
    out, v = [], start
    while v <= hi + step * 1e-9:
        out.append(round(v, 10)); v += step
    return out
