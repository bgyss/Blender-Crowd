//! Explicit provenance and redistribution authorization for M6 motion assets.

use serde::{Deserialize, Serialize};

pub const PROVENANCE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionAssetProvenanceV1 {
    pub schema_version: u32,
    pub asset_id: String,
    pub source_uri: String,
    pub content_hash: String,
    pub license_id: String,
    pub redistribution_allowed: bool,
    pub terms_reference: String,
    #[serde(default)]
    pub checkpoint_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProvenanceError {
    UnsupportedVersion,
    EmptyField(&'static str),
    InvalidContentHash,
    RedistributionNotAllowed,
    InvalidCheckpointHash,
    AbsoluteSourcePath,
}

impl MotionAssetProvenanceV1 {
    pub fn validate(&self) -> Result<(), ProvenanceError> {
        if self.schema_version != PROVENANCE_SCHEMA_VERSION {
            return Err(ProvenanceError::UnsupportedVersion);
        }
        for (field, value) in [
            ("asset_id", self.asset_id.as_str()),
            ("source_uri", self.source_uri.as_str()),
            ("license_id", self.license_id.as_str()),
            ("terms_reference", self.terms_reference.as_str()),
        ] {
            if value.is_empty() {
                return Err(ProvenanceError::EmptyField(field));
            }
        }
        if self.source_uri.starts_with('/') || self.source_uri.starts_with('~') {
            return Err(ProvenanceError::AbsoluteSourcePath);
        }
        if !is_hash(&self.content_hash) {
            return Err(ProvenanceError::InvalidContentHash);
        }
        if !self.redistribution_allowed {
            return Err(ProvenanceError::RedistributionNotAllowed);
        }
        if self
            .checkpoint_hash
            .as_deref()
            .is_some_and(|hash| !is_hash(hash))
        {
            return Err(ProvenanceError::InvalidCheckpointHash);
        }
        Ok(())
    }
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
