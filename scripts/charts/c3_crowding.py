"""Chart 3: the theory said dense clusters. The census said the crowd is sparse.

Linear x on purpose: a log axis would render 0.06% as a substantial bar beside
70.9% and flatter the tail. The point is that the tail is empty, so the tail
should look empty.
"""
import sys; sys.path.insert(0, '.')
from lib import *

BUCKETS = ["0", "1–2", "3–5", "6–9", "10–16", "17+"]
K10  = [70.885, 29.053, 0.0617, 0.0002, 0.0, 0.0]
K100 = [67.849, 32.054, 0.0971, 0.0,    0.0, 0.0]

W, H = 1240, 720
sv = Svg(W, H)
sv.text(48, 58, "The fix assumed dense clusters. There are none.", 27, INK, "700")
sv.text(48, 90, "Neighbours within personal space, counted the way the solver counts them. "
                "The proposed slowdown floor needs ~10 before it does anything at all.", 15, INK2)

X0, Y0, PW, PH = 132, 168, 830, 360
XMAX = 75.0
def px(v): return X0 + (v / XMAX) * PW

for t in (0, 10, 20, 30, 40, 50, 60, 70):
    x = px(t)
    sv.line(x, Y0 - 6, x, Y0 + PH, GRID, 1)
    sv.text(x, Y0 + PH + 24, f"{t}%", 12, MUTED, anchor="middle")
sv.text(X0 + PW / 2, Y0 + PH + 50, "share of observed agent-ticks", 13, MUTED, anchor="middle")
sv.text(X0 - 18, Y0 - 22, "neighbours", 12.5, MUTED, anchor="end")

rowh = PH / len(BUCKETS)
bh = 22
for i, b in enumerate(BUCKETS):
    cy = Y0 + rowh * (i + 0.5)
    sv.text(X0 - 18, cy + 5, b, 14, INK, "700", anchor="end")
    for j, (vals, color) in enumerate(((K10, S2), (K100, S1))):
        y = cy - bh - 1 + j * (bh + 2)
        v = vals[i]
        if v <= 0:
            sv.text(X0 + 4, y + bh - 5, "none observed", 12, MUTED, "700")
        elif v < 0.5:
            # Too small to draw honestly; a 3px stub plus the number, so the
            # row reads as "essentially nothing" rather than "missing".
            sv.rect(X0, y, 3, bh, color, r=1.5)
            sv.text(X0 + 12, y + bh - 5, f"{v:.4f}%", 12, MUTED, "700")
        else:
            sv.rect(X0, y, px(v) - X0, bh, color, r=4)
            sv.text(px(v) + 10, y + bh - 5, f"{v:.1f}%", 12.5, INK2, "700")
sv.line(X0, Y0 - 6, X0, Y0 + PH, "#cfcec8", 1.5)

band_y = Y0 + rowh * 4 - 2
sv.rect(X0 + 2, band_y, PW - 2, rowh * 2 - 4, "#f4ece9", r=6)
sv.text(X0 + PW - 14, band_y + 30, "a 0.35 speed floor only binds above ~10 neighbours", 13.5, "#a8412f", "700", anchor="end")
sv.text(X0 + PW - 14, band_y + 52, "no sample at either scale ever reaches this band", 13, "#a8412f", anchor="end")

LY = H - 108
sv.rect(132, LY, 15, 15, S2, r=3); sv.text(156, LY + 12, "10,000 agents — max 6 neighbours", 13.5, INK)
sv.rect(500, LY, 15, 15, S1, r=3); sv.text(524, LY + 12, "100,000 agents — max 4 neighbours", 13.5, INK)
sv.text(48, H - 46, "99.9% of agent-ticks have two neighbours or fewer. 100K is marginally sparser than 10K, not denser.", 13.5, MUTED)
sv.text(48, H - 24, "Where the floor did bind — on a genuinely dense scene — it made jams 74% longer. It ships disabled.", 13.5, MUTED)
sv.save("out_crowding.svg")
print("ok")
