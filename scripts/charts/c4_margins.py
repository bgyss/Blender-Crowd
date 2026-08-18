"""Chart 4: how much room the accepted 100K run has against each limit."""
import sys; sys.path.insert(0, '.')
from lib import *

# Measured / limit, from the accepted 100K adjudication (2026-08-18).
ROWS = [
    ("S2  animation evaluation share",  0.500,     0.75,    True),
    ("throughput (ticks/s)",            13.696,    10.0,    False),
    ("S2  stall agent-tick rate",       0.002870,  0.005,   True),
    ("S1  stall agent-tick rate",       0.002747,  0.005,   True),
    ("S1  stall episodes / agent-km",   0.503971,  0.9,     True),
    ("S2  stall episodes / agent-km",   0.492933,  0.9,     True),
    ("S1  heading reversals",           0.073216,  0.15,    True),
    ("S1  contact severity",            1.349e-8,  3e-8,    True),
    ("S1  contact rate",                8.272e-7,  2e-6,    True),
    ("S2  contact rate",                1.859e-6,  5e-6,    True),
    ("S2  contact severity",            3.368e-8,  1e-7,    True),
    ("S1  abrupt turns",                5.984e-6,  2e-5,    True),
    ("S2  heading reversals",           0.040448,  0.15,    True),
    ("S2  abrupt turns",                5.354e-6,  2e-5,    True),
]
rows = []
for name, measured, limit, at_most in ROWS:
    rows.append((name, limit / measured if at_most else measured / limit))
rows.sort(key=lambda r: r[1])

W, H = 1240, 812
sv = Svg(W, H)
sv.text(48, 58, "How much room the 100,000-agent run has, per limit", 27, INK, "700")
sv.text(48, 90, "Every gated check on the accepted run, as a multiple of the limit it had to clear. 1.0× is the limit itself.", 15, INK2)

X0, Y0, PW, PH = 340, 156, 620, 500
XMAX = 4.2
def px(v): return X0 + (v / XMAX) * PW

for t in (0, 1, 2, 3, 4):
    x = px(t)
    sv.line(x, Y0 - 6, x, Y0 + PH, GRID, 1)
    sv.text(x, Y0 + PH + 26, f"{t}×", 12.5, MUTED, anchor="middle")
sv.text(X0 + PW / 2, Y0 + PH + 52, "margin over the checked-in limit", 13, MUTED, anchor="middle")

rowh = PH / len(rows)
bh = min(rowh - 8, 22)
for i, (name, margin) in enumerate(rows):
    cy = Y0 + rowh * (i + 0.5)
    sv.text(X0 - 20, cy + 4.5, name, 13, INK2, anchor="end")
    sv.rect(X0, cy - bh / 2, px(margin) - X0, bh, S1, r=4)
    sv.text(px(margin) + 10, cy + 4.5, f"{margin:.2f}×", 12.5, INK, "700")

# The limit itself: everything to the left of this line would be a failure.
xl = px(1.0)
sv.line(xl, Y0 - 14, xl, Y0 + PH + 6, "#c9483f", 2, dash="6 5")
sv.text(xl + 8, Y0 - 20, "the limit — anything left of this fails", 13, "#c9483f", "700")
sv.line(X0, Y0 - 6, X0, Y0 + PH, "#cfcec8", 1.5)

sv.text(48, H - 62, "Tightest margin is 1.37× on throughput; the quality checks sit between 1.74× and 3.74×.", 13.5, MUTED)
sv.text(48, H - 40, "Not shown: two figures that are reported but never gated, because neither is scale-invariant —", 13.5, MUTED)
sv.text(48, H - 18, "a lifetime cumulative share and an extremum over samples both climb with population at unchanged behaviour.", 13.5, MUTED)
sv.save("out_margins.svg")
print("ok")
