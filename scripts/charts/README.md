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
