//! Versioned external behavior/action extension boundary.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub const EXTENSION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionChannelV1 {
    pub name: String,
    pub version: u32,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub cost_budget_millionths: u32,
    pub deterministic: bool,
    pub failure_isolated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionManifestV1 {
    pub schema_version: u32,
    pub id: String,
    pub channels: Vec<ExtensionChannelV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionValidationError {
    UnsupportedVersion(u32),
    UnknownChannel(String),
    UndeclaredInput(String),
    CostBudgetExceeded,
    NonDeterministic,
    NotFailureIsolated,
}

impl ExtensionManifestV1 {
    pub fn new(
        id: impl Into<String>,
        channels: Vec<ExtensionChannelV1>,
    ) -> Result<Self, Vec<String>> {
        let manifest = Self {
            schema_version: EXTENSION_SCHEMA_VERSION,
            id: id.into(),
            channels,
        };
        manifest.validate().map(|_| manifest)
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.schema_version != EXTENSION_SCHEMA_VERSION {
            errors.push(format!(
                "unsupported extension schema version {}",
                self.schema_version
            ));
        }
        if self.id.is_empty() {
            errors.push("extension manifest ID must be non-empty".to_owned());
        }
        let mut names = BTreeSet::new();
        for channel in &self.channels {
            if channel.name.is_empty() || !names.insert(channel.name.as_str()) {
                errors.push(format!(
                    "extension channel {} must have a unique non-empty name",
                    channel.name
                ));
            }
            if channel.version == 0 {
                errors.push(format!(
                    "extension channel {} must declare a positive version",
                    channel.name
                ));
            }
            if channel.inputs.iter().any(String::is_empty)
                || channel.outputs.iter().any(String::is_empty)
            {
                errors.push(format!(
                    "extension channel {} has an empty channel declaration",
                    channel.name
                ));
            }
            if channel.cost_budget_millionths == 0 {
                errors.push(format!(
                    "extension channel {} must declare a positive cost budget",
                    channel.name
                ));
            }
            if !channel.deterministic {
                errors.push(format!(
                    "extension channel {} must be deterministic in strict mode",
                    channel.name
                ));
            }
            if !channel.failure_isolated {
                errors.push(format!(
                    "extension channel {} must be failure isolated",
                    channel.name
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn validate_call(
        &self,
        channel_name: &str,
        inputs: &[&str],
        estimated_cost_millionths: u32,
    ) -> Result<(), ExtensionValidationError> {
        if self.schema_version != EXTENSION_SCHEMA_VERSION {
            return Err(ExtensionValidationError::UnsupportedVersion(
                self.schema_version,
            ));
        }
        let channel = self
            .channels
            .iter()
            .find(|channel| channel.name == channel_name)
            .ok_or_else(|| ExtensionValidationError::UnknownChannel(channel_name.to_owned()))?;
        if !channel.deterministic {
            return Err(ExtensionValidationError::NonDeterministic);
        }
        if !channel.failure_isolated {
            return Err(ExtensionValidationError::NotFailureIsolated);
        }
        for input in inputs {
            if !channel.inputs.iter().any(|declared| declared == input) {
                return Err(ExtensionValidationError::UndeclaredInput(
                    (*input).to_owned(),
                ));
            }
        }
        if estimated_cost_millionths > channel.cost_budget_millionths {
            return Err(ExtensionValidationError::CostBudgetExceeded);
        }
        Ok(())
    }
}
