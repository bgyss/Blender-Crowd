//! Reusable, bounded action definitions for M6 brain authoring.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionDefinitionV1 {
    pub id: String,
    pub channel: String,
    pub cost_millionths: u32,
    pub fallback_id: String,
}

impl ActionDefinitionV1 {
    pub fn new(
        id: impl Into<String>,
        channel: impl Into<String>,
        cost_millionths: u32,
        fallback_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            channel: channel.into(),
            cost_millionths,
            fallback_id: fallback_id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionLibraryV1 {
    actions: BTreeMap<String, ActionDefinitionV1>,
}

impl ActionLibraryV1 {
    pub fn new(actions: Vec<ActionDefinitionV1>) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let mut by_id = BTreeMap::new();
        for action in actions {
            if action.id.is_empty() || action.channel.is_empty() || action.fallback_id.is_empty() {
                errors.push(format!(
                    "action {} must declare ID, channel, and fallback",
                    action.id
                ));
            }
            if action.cost_millionths == 0 {
                errors.push(format!("action {} must declare a positive cost", action.id));
            }
            if by_id.insert(action.id.clone(), action).is_some() {
                errors.push("duplicate action ID".to_owned());
            }
        }
        for action in by_id.values() {
            if !by_id.contains_key(&action.fallback_id) {
                errors.push(format!(
                    "action {} references missing fallback {}",
                    action.id, action.fallback_id
                ));
            }
        }
        if errors.is_empty() {
            Ok(Self { actions: by_id })
        } else {
            Err(errors)
        }
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn ids(&self) -> Vec<String> {
        self.actions.keys().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<&ActionDefinitionV1> {
        self.actions.get(id)
    }
}
