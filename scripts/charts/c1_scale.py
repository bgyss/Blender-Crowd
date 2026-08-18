"""Chart 1: the same run, two normalisations. One crosses its bar, one does not."""
import sys; sys.path.insert(0, '.')
from lib import *

# S2 tier, measured. Sources: schema-v6 reports at 1K/10K/20K/40K and the
# accepted 100K report (2026-08-18).
AGENTS = [1000, 10000, 20000, 40000, 100000]
SHARE  = [0.0523, 0.1023, 0.1154, 0.1490, 0.2599]   # stalled_agent_share
PERKM  = [0.2917, 0.2202, 0.1748, 0.2060, 0.4929]   # stall_episodes_per_agent_km
SHARE_BAR, PERKM_BAR = 0.20, 0.9

W, H = 1240, 720
sv = Svg(W, H)
sv.text(48, 58, "One run, two ways of counting the same stalls", 27, INK, "700")
sv.text(48, 90, "Both panels describe the identical 100,000-agent simulation. Only the left one says it got worse.", 15, INK2)

PW, PH = 500, 380
PY0, PX = 168, [48, 660]
TITLES = [("Share of agents that ever stalled", "a lifetime cumulative probability", S2, SHARE, SHARE_BAR, "gate limit 0.20"),
          ("Stall episodes per agent-kilometre", "a rate per distance actually walked", S1, PERKM, PERKM_BAR, "gate limit 0.9")]

for i, (title, sub, color, vals, bar, barlabel) in enumerate(TITLES):
    x0 = PX[i]
    ymax = max(max(vals), bar) * 1.18
    sv.text(x0, PY0 - 40, title, 17, INK, "700")
    sv.text(x0, PY0 - 18, sub, 14, MUTED)

    # Log-scaled x: population spans two decades, so even spacing would
    # distort the shape of the line it is drawn through.
    import math as _m
    lo, hi = _m.log10(AGENTS[0]), _m.log10(AGENTS[-1])
    def px(j):
        f = (_m.log10(AGENTS[j]) - lo) / (hi - lo)
        return x0 + 62 + f * (PW - 82)
    def py(v): return PY0 + PH - (v / ymax) * PH

    for t in nice_ticks(0, ymax, 4):
        y = py(t)
        sv.line(x0 + 50, y, x0 + PW, y, GRID, 1)
        sv.text(x0 + 42, y + 4, f"{t:.2f}".rstrip("0").rstrip("."), 12, MUTED, anchor="end")

    # The gate limit this metric was judged against.
    yb = py(bar)
    sv.line(x0 + 50, yb, x0 + PW, yb, "#c9483f", 2, dash="6 5")
    sv.text(x0 + 56, yb - 9, barlabel, 12.5, "#c9483f", "700", anchor="start")

    d = "M " + " L ".join(f"{px(j):.1f} {py(v):.1f}" for j, v in enumerate(vals))
    sv.path(d, color, 2.5)
    for j, v in enumerate(vals):
        sv.dot(px(j), py(v), 5, color)
    # Direct-label the endpoints only, not every point.
    sv.text(px(0), py(vals[0]) - 16, f"{vals[0]:.3f}", 12.5, INK2, "700", anchor="middle")
    over = vals[-1] > bar
    sv.text(px(len(vals)-1), py(vals[-1]) - 16, f"{vals[-1]:.3f}",
            13.5, "#c9483f" if over else INK, "700", anchor="end")

    for j, a in enumerate(AGENTS):
        lab = f"{a//1000}K" if a >= 1000 else str(a)
        sv.text(px(j), PY0 + PH + 24, lab, 12.5, MUTED, anchor="middle")
    sv.text(x0 + 50 + (PW - 50) / 2, PY0 + PH + 50, "agents (log scale)", 13, MUTED, anchor="middle")

    fold = vals[-1] / vals[0]
    verdict = "FAILS the limit at 100K" if over else "inside the limit at 100K"
    sv.text(x0, PY0 + PH + 92, f"\u00d7{fold:.1f} from 1K to 100K", 15.5, INK, "700")
    sv.text(x0, PY0 + PH + 116, verdict, 14.5, "#c9483f" if over else INK2, "700")

sv.line(620, 150, 620, 606, GRID, 1)
sv.text(48, H - 30,
        "Routes grow with the square root of population, so a lifetime share climbs toward 1.0 even at a constant blocking rate per metre.", 13.5, MUTED)
sv.text(48, H - 11, "Dividing by distance actually walked removes that exposure. Same simulation, same stalls \u2014 only the denominator differs.", 13.5, MUTED)
sv.save("out_scale.svg")
print("ok")
