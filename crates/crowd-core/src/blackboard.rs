//! Typed, bounded blackboard values and deterministic fuzzy predicates.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::AgentId;

pub const MAX_BLACKBOARD_TEXT_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlackboardTypeV1 {
    Bool,
    NumberI32,
    Enum,
    AgentId,
    Text,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum BlackboardValueV1 {
    Bool(bool),
    NumberI32(i32),
    Enum(String),
    AgentId(AgentId),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlackboardChannelV1 {
    pub key: String,
    pub value_type: BlackboardTypeV1,
    pub default: BlackboardValueV1,
}

impl BlackboardChannelV1 {
    pub fn new(
        key: impl Into<String>,
        value_type: BlackboardTypeV1,
        default: BlackboardValueV1,
    ) -> Self {
        Self {
            key: key.into(),
            value_type,
            default,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlackboardError {
    EmptyKey,
    DuplicateKey(String),
    DefaultTypeMismatch(String),
    Undeclared(String),
    TypeMismatch {
        key: String,
        expected: BlackboardTypeV1,
    },
    TextTooLong(String),
}

impl fmt::Display for BlackboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => write!(f, "blackboard keys must be non-empty"),
            Self::DuplicateKey(key) => write!(f, "blackboard key {key} is declared more than once"),
            Self::DefaultTypeMismatch(key) => write!(
                f,
                "blackboard key {key} has a default value with the wrong type"
            ),
            Self::Undeclared(key) => write!(f, "blackboard key {key} is undeclared"),
            Self::TypeMismatch { key, expected } => {
                write!(f, "blackboard key {key} expects {expected:?}")
            }
            Self::TextTooLong(key) => write!(
                f,
                "blackboard text value for {key} exceeds {MAX_BLACKBOARD_TEXT_BYTES} bytes"
            ),
        }
    }
}

impl Error for BlackboardError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlackboardChangeV1 {
    pub key: String,
    pub previous: BlackboardValueV1,
    pub current: BlackboardValueV1,
}

#[derive(Clone, Debug)]
pub struct BlackboardStateV1 {
    channels: BTreeMap<String, BlackboardChannelV1>,
    values: BTreeMap<String, BlackboardValueV1>,
    changes: BTreeMap<String, (BlackboardValueV1, BlackboardValueV1)>,
}

impl BlackboardStateV1 {
    pub fn new(channels: Vec<BlackboardChannelV1>) -> Result<Self, BlackboardError> {
        let mut definitions = BTreeMap::new();
        let mut values = BTreeMap::new();
        for channel in channels {
            if channel.key.is_empty() {
                return Err(BlackboardError::EmptyKey);
            }
            if definitions.contains_key(&channel.key) {
                return Err(BlackboardError::DuplicateKey(channel.key));
            }
            if !matches_type(&channel.default, channel.value_type)
                || matches!(&channel.default, BlackboardValueV1::Text(text) if text.len() > MAX_BLACKBOARD_TEXT_BYTES)
            {
                return Err(BlackboardError::DefaultTypeMismatch(channel.key));
            }
            values.insert(channel.key.clone(), channel.default.clone());
            definitions.insert(channel.key.clone(), channel);
        }
        Ok(Self {
            channels: definitions,
            values,
            changes: BTreeMap::new(),
        })
    }

    pub fn get(&self, key: &str) -> Option<&BlackboardValueV1> {
        self.values.get(key)
    }

    pub fn set(&mut self, key: &str, value: BlackboardValueV1) -> Result<(), BlackboardError> {
        let channel = self
            .channels
            .get(key)
            .ok_or_else(|| BlackboardError::Undeclared(key.to_owned()))?;
        if !matches_type(&value, channel.value_type) {
            return Err(BlackboardError::TypeMismatch {
                key: key.to_owned(),
                expected: channel.value_type,
            });
        }
        if matches!(&value, BlackboardValueV1::Text(text) if text.len() > MAX_BLACKBOARD_TEXT_BYTES)
        {
            return Err(BlackboardError::TextTooLong(key.to_owned()));
        }
        let previous = self
            .values
            .get(key)
            .expect("every declared key has a value")
            .clone();
        if previous == value {
            return Ok(());
        }
        self.values.insert(key.to_owned(), value.clone());
        self.changes
            .entry(key.to_owned())
            .and_modify(|(_, current)| *current = value.clone())
            .or_insert((previous, value));
        Ok(())
    }

    pub fn drain_changes(&mut self) -> Vec<BlackboardChangeV1> {
        std::mem::take(&mut self.changes)
            .into_iter()
            .map(|(key, (previous, current))| BlackboardChangeV1 {
                key,
                previous,
                current,
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FuzzyComparisonV1 {
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
    Equal,
    BetweenInclusive(i32, i32),
}

pub fn fuzzy_membership(value: i32, lower: i32, upper: i32) -> u32 {
    if lower >= upper {
        return if value >= upper { 1_000_000 } else { 0 };
    }
    if value <= lower {
        0
    } else if value >= upper {
        1_000_000
    } else {
        (((value - lower) as i64 * 1_000_000) / (upper - lower) as i64) as u32
    }
}

pub fn fuzzy_compare(value: i32, comparison: FuzzyComparisonV1, threshold: i32) -> bool {
    match comparison {
        FuzzyComparisonV1::LessThan => value < threshold,
        FuzzyComparisonV1::LessOrEqual => value <= threshold,
        FuzzyComparisonV1::GreaterThan => value > threshold,
        FuzzyComparisonV1::GreaterOrEqual => value >= threshold,
        FuzzyComparisonV1::Equal => value == threshold,
        FuzzyComparisonV1::BetweenInclusive(lower, upper) => value >= lower && value <= upper,
    }
}

fn matches_type(value: &BlackboardValueV1, value_type: BlackboardTypeV1) -> bool {
    matches!(
        (value, value_type),
        (BlackboardValueV1::Bool(_), BlackboardTypeV1::Bool)
            | (BlackboardValueV1::NumberI32(_), BlackboardTypeV1::NumberI32)
            | (BlackboardValueV1::Enum(_), BlackboardTypeV1::Enum)
            | (BlackboardValueV1::AgentId(_), BlackboardTypeV1::AgentId)
            | (BlackboardValueV1::Text(_), BlackboardTypeV1::Text)
    )
}
