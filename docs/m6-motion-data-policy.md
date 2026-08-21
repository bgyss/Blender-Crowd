# M6 motion and trajectory data policy

> **Status (2026-08-20).** This policy stays in force unchanged. The goal it
> serves — acquiring a production motion corpus that satisfies the unchanged M6
> motion thresholds — moved with acceptance criterion 5 to
> [M9 Track C](milestones/M9-neural-animation-operator-validation.md) because
> the acquisition has no schedule. The CMU candidate remains rejected at 3,587
> measured joint-limit violations against a hard limit of zero. Any future
> corpus is ingested under exactly these provenance, licensing, and
> redistribution rules. See the
> [deferral record](benchmarks/2026-08-20-m6-criterion-5-deferral.md).

M6 may ingest motion or trajectory data only when the repository has a
versioned provenance manifest with:

- stable asset identity and content hash;
- source URI that is repository-relative or otherwise machine-neutral;
- license identifier and a durable terms reference;
- explicit redistribution status and repository/package boundaries;
- checkpoint/configuration identity when a learned artifact is involved.

The checked M6 foundation uses redistributable reference metadata, authored
paired-clip fixtures, and one narrowly bounded CMU evidence lane. The official
CMU Motion Capture Database terms permit all uses, including inclusion in
commercial products, but prohibit direct resale of the data. Consequently,
the checked CMU manifest records `redistribution_allowed: false`: raw ASF/AMC
and converted databases must remain outside Git, while fixed URLs and hashes,
fetch/import code, hand-authored mini fixtures, thresholds, and derived
aggregate reports may be committed.

CMU acquisition is allowed only through the versioned five-file source
manifest. The fetcher must refuse unknown hosts, off-host redirects, changed or
extra files, changed IDs or clip/subject/trial/skeleton relationships, and hash
mismatch before publication. Production ingest applies the same fixed manifest
validator. Hand-authored mini ASF/AMC parser tests use a separate explicitly
non-production fixture entry point and cannot confer CMU provenance. Evidence
runs use a temporary artifact directory and never Blender's user profile. This
exception does not authorize other motion corpora, neural checkpoints, game
assets, or paid/cloud data. A permissively licensed code repository is not by
itself evidence that its datasets, checkpoints, or source animations may be
used or redistributed.

An asset without a valid manifest must be rejected before database build,
retargeting, worker invocation, Blender packaging, or cache publication. A
manifest with `redistribution_allowed: false` must also keep every raw and
converted file outside repository and package boundaries. The deterministic
clip-state baseline remains available when provenance, hardware, or licensing
requirements are not satisfied.
