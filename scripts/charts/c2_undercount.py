"""Chart 2: the background tier's contact rate was halved by its own schedule."""
import sys; sys.path.insert(0, '.')
from lib import *

# S2 tier. biased = contacts / all agent-ticks (the old denominator);
# fixed = contacts / ticks on which neighbours were actually looked up.
AGENTS = ["10K", "20K", "40K", "100K"]
BIASED = [6.549e-7, 4.792e-7, 5.334e-7, 9.295e-7]
FIXED  = [1.310e-6, 9.584e-7, 1.067e-6, 1.859e-6]

W, H = 1240, 700
sv = Svg(W, H)
sv.text(48, 58, "A background agent can only be seen colliding half the time", 27, INK, "700")
sv.text(48, 90, "Agents on a 2-tick perception cadence got an empty neighbour list on skipped ticks — "
                "but every tick still counted as clean exposure.", 15, INK2)

X0, Y0, PW, PH = 96, 190, 740, 380
ymax = max(FIXED) * 1.22

def py(v): return Y0 + PH - (v / ymax) * PH
for t in nice_ticks(0, ymax, 5):
    y = py(t)
    sv.line(X0, y, X0 + PW, y, GRID, 1)
    sv.text(X0 - 12, y + 4, f"{t*1e6:.1f}", 12, MUTED, anchor="end")
sv.text(X0 - 60, Y0 - 26, "contacts per million agent-ticks", 13, MUTED, anchor="start")

group = PW / len(AGENTS)
bw = 76
for i, a in enumerate(AGENTS):
    cx = X0 + group * (i + 0.5)
    # 2px surface gap between adjacent bars (marks-and-anatomy).
    for j, (vals, color) in enumerate(((BIASED, S2), (FIXED, S1))):
        x = cx - bw - 1 + j * (bw + 2)
        y = py(vals[i])
        sv.rect(x, y, bw, Y0 + PH - y, color, r=4)
        sv.text(x + bw / 2, y - 10, f"{vals[i]*1e6:.2f}", 12.5, INK2, "700", anchor="middle")
    sv.text(cx, Y0 + PH + 26, a, 14, INK2, "700", anchor="middle")
sv.line(X0, Y0 + PH, X0 + PW, Y0 + PH, "#cfcec8", 1.5)
sv.text(X0 + PW / 2, Y0 + PH + 52, "agents", 13, MUTED, anchor="middle")

# Legend: always present for two series.
LX, LY = 940, 230
sv.text(LX, LY - 22, "S2 contact rate", 15, INK, "700")
for j, (lab, color, note) in enumerate((
        ("as measured before", S2, "divided by every tick"),
        ("corrected", S1, "divided by observed ticks"))):
    y = LY + j * 56
    sv.rect(LX, y, 16, 16, color, r=3)
    sv.text(LX + 26, y + 13, lab, 14, INK, "700")
    sv.text(LX + 26, y + 32, note, 12.5, MUTED)

sv.rect(LX, LY + 132, 244, 2, GRID)
sv.text(LX, LY + 168, "Exactly 2×, at every scale", 16, INK, "700")
sv.text(LX, LY + 192, "S2 is observed on 0.500 of its ticks.", 13, INK2)
sv.text(LX, LY + 212, "S1 perceives every tick: 1.000,", 13, INK2)
sv.text(LX, LY + 232, "and was never affected.", 13, INK2)

sv.text(48, H - 52, "Found by attribution: the same S1–S2 collisions were counted 65 times from the S1 side and 35 from the S2 side.", 13.5, MUTED)
sv.text(48, H - 30, "Every per-tier contact figure published before 2026-08-17 is understated 2× and is not comparable with anything measured after it.", 13.5, MUTED)
sv.save("out_undercount.svg")
print("ok")
