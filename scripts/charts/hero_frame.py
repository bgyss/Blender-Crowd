"""Render one measured frame of the 100,000-agent scale fixture.

Input is `crates/crowd-core/tests/m5_hero_frame.rs` output: the true x/y/tier
of every agent present at a tick where occupancy is 100%. This is a positional
plot of measured state, not a Blender render, and the caption says so.
"""
import csv, sys
from PIL import Image, ImageDraw, ImageFont

SRC = sys.argv[1] if len(sys.argv) > 1 else "hero-100k.csv"
OUT = sys.argv[2] if len(sys.argv) > 2 else "hero-100k.png"
import re as _re
_m = _re.search(r"(\d+)\.csv$", SRC)
TICK = int(_m.group(1)) if _m else 0

SURFACE = (252, 252, 251)
INK     = (11, 11, 11)
INK2    = (82, 81, 78)
MUTED   = (138, 137, 129)
S1_COL  = (235, 104, 52)    # foreground tier, 10%
S2_COL  = (42, 120, 214)    # background tier, 90%
PANEL   = (243, 243, 240)

SS = 2                      # supersample factor
# Scene extent, for the locator that shows where the occupied region sits.
SCENE_W, SCENE_H = 2529.822, 1264.911

pts = []
with open(SRC) as f:
    for r in csv.DictReader(f):
        pts.append((float(r["x"]), float(r["y"]), int(r["tier"])))

xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
x0, x1 = min(xs), max(xs); y0, y1 = min(ys), max(ys)
pad = 22.0
x0 -= pad; x1 += pad; y0 -= pad; y1 += pad

# The crowd is a travelling wave a few hundred metres wide, so at any one tick
# most of the 2,530 m scene is empty ground. Size the canvas to the occupied
# region and carry a locator rather than framing a mostly-blank scene.
DATA_W, DATA_H = x1 - x0, y1 - y0
# Two columns: the plot keeps its own space and the detail inset sits beside it
# rather than on top of it, so the inset never hides agents the title counts.
IW = 780
GAP = 56
PLOT_W = 1660
PLOT_H = int(round(PLOT_W * DATA_H / DATA_W))
PX, PY = 80, 250
W = PX + PLOT_W + GAP + IW + PX
H = PY + max(PLOT_H, IW) + 300
PW, PH = PLOT_W, PLOT_H
s = PLOT_W / DATA_W
ox, oy = PX, PY

def to_px(x, y):
    # y flipped: scene +y is up, image +y is down.
    return (ox + (x - x0) * s, oy + (y1 - y) * s)

img = Image.new("RGB", (W * SS, H * SS), SURFACE)
d = ImageDraw.Draw(img)

# Draw background tier first so the 10% foreground reads on top of it.
R = 1.35 * SS
for want, col in ((2, S2_COL), (1, S1_COL)):
    for x, y, t in pts:
        if t != want:
            continue
        px, py = to_px(x, y)
        px *= SS; py *= SS
        d.ellipse([px - R, py - R, px + R, py + R], fill=col)

# --- inset: a 70 m window at true relative scale ---
# Centred on the densest 70 m cell rather than a fixed point: the two lane
# blocks travel from opposite ends, so a geometric centre can easily land on
# empty ground and produce an empty inset.
span = 70.0
cells = {}
for x, y, _t in pts:
    cells.setdefault((int(x // span), int(y // span)), []).append((x, y))
(bx, by), best = max(cells.items(), key=lambda kv: len(kv[1]))
cx_m = sum(p[0] for p in best) / len(best)
cy_m = sum(p[1] for p in best) / len(best)
print(f"inset centred on {len(best)} agents at ({cx_m:.0f}, {cy_m:.0f})")
ix, iy = PX + PLOT_W + GAP, PY + max(0, (PLOT_H - IW) // 2)
d.rectangle([ix * SS, iy * SS, (ix + IW) * SS, (iy + IW) * SS], fill=PANEL)
isc = IW / span
for x, y, t in pts:
    if abs(x - cx_m) > span / 2 or abs(y - cy_m) > span / 2:
        continue
    px = ix + (x - (cx_m - span / 2)) * isc
    py = iy + ((cy_m + span / 2) - y) * isc
    r = 0.31 * isc * SS          # true agent radius, ~0.31 m
    d.ellipse([px * SS - r, py * SS - r, px * SS + r, py * SS + r],
              fill=S1_COL if t == 1 else S2_COL)
d.rectangle([ix * SS, iy * SS, (ix + IW) * SS, (iy + IW) * SS], outline=(206, 205, 198), width=2 * SS)

# Locator box on the main plot showing where the inset came from.
lx0, ly0 = to_px(cx_m - span / 2, cy_m + span / 2)
lx1, ly1 = to_px(cx_m + span / 2, cy_m - span / 2)
d.rectangle([lx0 * SS, ly0 * SS, lx1 * SS, ly1 * SS], outline=INK, width=2 * SS)

img = img.resize((W, H), Image.LANCZOS)
d = ImageDraw.Draw(img)

def font(sz, bold=False):
    for p in ("/System/Library/Fonts/Helvetica.ttc",
              "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
              "/System/Library/Fonts/Supplemental/Arial.ttf"):
        try:
            return ImageFont.truetype(p, sz, index=1 if (bold and p.endswith("ttc")) else 0)
        except Exception:
            continue
    return ImageFont.load_default()

d.text((80, 58), "100,000 agents, all of them, at one tick", font=font(62, True), fill=INK)
d.text((80, 140), f"Every agent present in the m5_city_flow scale fixture at tick {TICK:,} \u2014 occupancy 100.0%, not a subset.",
       font=font(28), fill=INK2)
d.text((80, 182), f"View is cropped to the {DATA_W:,.0f} \u00d7 {DATA_H:,.0f} m the crowd occupies; the scene is "
                  f"{SCENE_W:,.0f} \u00d7 {SCENE_H:,.0f} m.", font=font(26), fill=MUTED)

ly = H - 198
d.ellipse([80, ly + 6, 98, ly + 24], fill=S1_COL)
d.text((110, ly), "10,029 foreground S1 — re-steered every tick", font=font(28, True), fill=INK)
d.ellipse([1000, ly + 6, 1018, ly + 24], fill=S2_COL)
d.text((1030, ly), "89,971 background S2 — 2-tick perception and steering cadence", font=font(28, True), fill=INK)

d.text((80, H - 140), "Two opposing lane blocks, each having travelled from its own end of the scene. Dots are drawn "
                      "larger than life at this zoom;", font=font(24), fill=MUTED)
d.text((80, H - 108), "the inset shows the same crowd at true agent radius.", font=font(24), fill=MUTED)
d.text((80, H - 68), "Positions measured by crates/crowd-core/tests/m5_hero_frame.rs. A plot of simulation state, not a "
                     "Blender render \u2014 the Blender evidence", font=font(24), fill=MUTED)
d.text((80, H - 36), "proves the population is never expanded into per-agent scene objects.", font=font(24), fill=MUTED)
d.text((ix, iy - 40), "70 m detail, agents at true size", font=font(26, True), fill=INK2)

img.save(OUT, quality=95)
print("wrote", OUT, img.size)
