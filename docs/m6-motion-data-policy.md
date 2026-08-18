# M6 motion and trajectory data policy

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
extra files, and hash mismatch before publication. Evidence runs use a
temporary artifact directory and never Blender's user profile. This exception
does not authorize other motion corpora, neural checkpoints, game assets, or
paid/cloud data. A permissively licensed code repository is not by itself
evidence that its datasets, checkpoints, or source animations may be used or
redistributed.

An asset without a valid manifest must be rejected before database build,
retargeting, worker invocation, Blender packaging, or cache publication. A
manifest with `redistribution_allowed: false` must also keep every raw and
converted file outside repository and package boundaries. The deterministic
clip-state baseline remains available when provenance, hardware, or licensing
requirements are not satisfied.
