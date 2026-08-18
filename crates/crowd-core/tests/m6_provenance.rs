use crowd_core::provenance::{MotionAssetProvenanceV1, ProvenanceError};

fn manifest() -> MotionAssetProvenanceV1 {
    MotionAssetProvenanceV1 {
        schema_version: 1,
        asset_id: "reference-walk-metadata".to_owned(),
        source_uri: "repo://assets/reference/m6/motion-database-input-v1.json".to_owned(),
        content_hash: "a".repeat(64),
        license_id: "CC0-1.0".to_owned(),
        redistribution_allowed: true,
        terms_reference: "docs/m6-motion-data-policy.md".to_owned(),
        checkpoint_hash: None,
    }
}

#[test]
fn provenance_manifest_requires_license_redistribution_and_content_identity() {
    let manifest = manifest();
    manifest.validate().unwrap();
    let mut invalid = manifest.clone();
    invalid.redistribution_allowed = false;
    assert_eq!(
        invalid.validate(),
        Err(ProvenanceError::RedistributionNotAllowed)
    );
    invalid = manifest;
    invalid.content_hash = "bad".to_owned();
    assert_eq!(invalid.validate(), Err(ProvenanceError::InvalidContentHash));
}

#[test]
fn checkpoint_provenance_is_optional_for_reference_metadata_but_validated_when_present() {
    let mut manifest = manifest();
    manifest.checkpoint_hash = Some("b".repeat(64));
    manifest.validate().unwrap();
    manifest.checkpoint_hash = Some("not-a-hash".to_owned());
    assert_eq!(
        manifest.validate(),
        Err(ProvenanceError::InvalidCheckpointHash)
    );
}
