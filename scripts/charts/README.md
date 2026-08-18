# Announcement charts

Regenerates the figures in `docs/media/m5-100k-*.png` from the measured M5
evidence. Every number in these scripts is transcribed from a checked-in
report; the source for each is named in the script's header comment.

```sh
cd scripts/charts
python3 c1_scale.py && magick -density 144 out_scale.svg      ../../docs/media/m5-100k-scale-invariance.png
python3 c2_undercount.py && magick -density 144 out_undercount.svg ../../docs/media/m5-100k-contact-undercount.png
python3 c3_crowding.py && magick -density 144 out_crowding.svg ../../docs/media/m5-100k-crowding.png
python3 c4_margins.py && magick -density 144 out_margins.svg   ../../docs/media/m5-100k-gate-margins.png
```

Needs Python 3 and ImageMagick (`magick`); no plotting library.

Palette is the dataviz reference instance, light mode, categorical slots 1-2
(`#2a78d6`, `#eb6834`), validated: lightness band, chroma floor, CVD separation
(worst adjacent ΔE 24.7 protan), normal-vision floor (ΔE 33.6), and contrast
against the surface all pass.

Two deliberate form choices, both about not flattering the data:

- `c3_crowding.py` uses a **linear** x-axis. On a log axis the 0.06% bucket
  renders as a substantial bar beside 70.9%, which would make an empty tail look
  populated. The point of the chart is that the tail is empty, so it is drawn
  empty, with the tiny values labelled rather than scaled up.
- `c1_scale.py` uses a **log** x-axis, because population spans two decades and
  even spacing would distort the shape of the line drawn through it.

## Hero frame

`hero_frame.py` renders one measured frame of the 100,000-agent fixture. Its
input comes from a probe that dumps true agent state at chosen ticks; see the
header of `crates/crowd-core/tests/m5_hero_frame.rs` for the exact invocation.
Run it detached (`nohup`) — it simulates to the requested tick, about 25 minutes
at 100K for tick 20,000.

The tick matters, for two separate reasons.

**Occupancy.** Every agent is present from a few ticks after emission until the
first arrivals at roughly tick 42,000, so a frame from inside that window is the
whole population rather than a subset. The probe asserts this and fails rather
than emitting a flattering frame if occupancy drops below 95%. This is the
difference between this image and the Blender scale capture, which shows 1,200
of 100,000 because the addon's reference scene emits slowly.

**Composition.** The crowd is a travelling wave a few hundred metres wide, and
the two opposing lane blocks start at opposite ends of the scene, so most ticks
put them in separate corners with empty ground between. They overlap around tick
19,000, which is why 20,000 is used.

Three things the image states about itself, because each could otherwise
mislead: the view is cropped to the region the crowd occupies (the scene is
larger and mostly empty at any one tick); dots are drawn larger than life at
full-scene zoom, with the inset at true agent radius; and it is a plot of
simulation state, not a Blender render. The inset auto-centres on the densest
70 m cell, since a fixed centre lands on empty ground at most ticks.

The banding visible in the plot is real lane structure, not a dot-size artifact:
the y-histogram shows peaks of 120-147 agents every ~2.5 m against 20-40
between, matching the fixture's 2.663 m lane pitch.
