# M6 motion and trajectory data policy

M6 may ingest motion or trajectory data only when the repository has a
versioned provenance manifest with:

- stable asset identity and content hash;
- source URI that is repository-relative or otherwise machine-neutral;
- license identifier and a durable terms reference;
- explicit redistribution authorization;
- checkpoint/configuration identity when a learned artifact is involved.

The checked M6 foundation uses only redistributable reference metadata and
authored paired-clip fixtures. It does not download motion corpora, neural
checkpoints, game assets, or paid/cloud data. A permissively licensed code
repository is not by itself evidence that its datasets, checkpoints, or source
animations may be redistributed.

An asset without a valid manifest must be rejected before database build,
retargeting, worker invocation, Blender packaging, or cache publication. The
deterministic clip-state baseline remains available when provenance, hardware,
or licensing requirements are not satisfied.

