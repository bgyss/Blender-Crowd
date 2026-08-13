# Artifact storage policy

Git history is for source, schemas, small deterministic fixtures, benchmark
baselines, and compact reports. Generated cache directories are not source and
must not be committed. In particular, `m2-crowd-cache/` and `m2-proof/` are
ignored because a complete reference bake can contain hundreds of megabytes of
frame and event data.

## Small GitHub fixtures

A future release may add a separate, self-contained demonstration project with
about 100 agents. Keep it deterministic, document the runner and environment,
and keep the checked-in fixture deliberately small enough for ordinary clone,
review, and CI use. It should prove format and workflow behavior rather than
serve as a scale benchmark.

## Scale evidence

The 1,000-agent acceptance cache, 10,000-agent fixtures, 100,000-agent
fixtures, long recordings, renders, and other large generated outputs belong
on a purpose-built artifact service or a separate sample-project repository,
not GitHub source history or Git LFS. Store immutable checksums, provenance,
environment details, and compact benchmark/acceptance reports in this
repository so external artifacts remain auditable and reproducible.

Do not make a performance or compatibility claim merely because an external
artifact exists. The checked-in runner and recorded evidence remain the source
of the claim.
