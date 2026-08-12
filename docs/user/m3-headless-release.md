# M3 archive-first headless acceptance

Do not use a checkout pass as release evidence. Build one candidate archive on
the target platform, then test that exact file:

```sh
scripts/m3-build-release.sh --out /tmp/blender-crowd-1.0-macos-arm64
scripts/m3-acceptance.sh \
  --archive /tmp/blender-crowd-1.0-macos-arm64/blender_crowd-1.0.0.zip \
  --out /tmp/blender-crowd-m3-proof
```

The acceptance runner audits the archive first, performs the complete 10,000
tick reference bake/cache-only replay/debug/override/render workflow, and runs
the save/reload/canceled-cache/support-bundle recovery drill. Preserve the
output directory as release evidence. Run it once for every support-matrix row.

The command passes only the archive into Blender installation; source fixtures
remain available solely to the test harness for fixture identity checks. A
passing run is still not M3 acceptance until support-matrix budgets, signing,
license review, and two independent evaluator records are complete.
