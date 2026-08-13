# Blender Crowd USD profile v1

This M4 profile exchanges a composed, non-destructive crowd layer result. It is
not a claim that every USD consumer understands every Blender Crowd feature.

## Contract

- A `PointInstancer` stores one point per stable agent; scene-object expansion
  is forbidden by this profile.
- `ids` are the stable Crowd agent IDs. `crowd:variant` carries the selected
  prototype/appearance variant. `positions` are the composed positions.
- Root `customLayerData` contains `crowdProfile = BlenderCrowd/v1` and the
  immutable `baseCacheHash`. This is a BLAKE3 identity over the complete Cache
  v1 manifest, agent table, chunks, and optional behavior evidence—not merely
  the source-project hash. Importers must reject a missing or mismatched hash,
  rather than attaching layer opinions to another bake.
- This first writer preserves identities, transforms, variant choices, and the
  profile/base-cache provenance. Animation samples, physics handoff metadata,
  path guides, and unresolved layer conflicts are deliberately unsupported in
  this compact interchange profile and must be surfaced as export warnings.

The cache-side procedural extraction contract retains those same presentation
channels for render-time instancing: identity, transform, variant-to-prototype
and material selection, clip/phase, visibility, and render tier. It never
creates a persistent Blender object per agent.

## Validation boundary

`crowd_cache::write_usda_crowd_profile_v1` and
`crowd_cache::read_usda_crowd_profile_v1` have deterministic round-trip
coverage for the profile’s stable IDs, positions, variants, and base hash. The
checked local suite also invokes OpenUSD's `usdcat --loadOnly` and `usdchecker`
when those tools are installed. This proves the narrow OpenUSD profile path; it
does not claim Blender, Houdini, or Unreal feature compatibility.
