//! Versioned, model-independent interaction requests and validated motion.
//!
//! The deterministic crowd runtime owns roles, roots, contacts, outcomes, and
//! fallback decisions. A worker may propose skeletal motion through these
//! types, but it cannot mutate the base simulation or decide an outcome.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const INTERACTION_REQUEST_SCHEMA_VERSION: u32 = 1;
pub const INTERACTION_MOTION_SCHEMA_VERSION: u32 = 1;
const MAX_ROOT_STEP_M: f32 = 2.0;
const MAX_ROOT_DEVIATION_M: f32 = 0.25;
const MAX_CONTACT_DISTANCE_M: f32 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionGroupStatusV1 {
    Queued,
    Promoted,
    Completed,
    Fallback,
}

#[derive(Clone, Debug)]
struct InteractionGroupStateV1 {
    request: InteractionRequestV1,
    status: InteractionGroupStatusV1,
}

/// Deterministic promotion and failure isolation for multi-agent interactions.
///
/// A participant is locked to the group for its whole promoted interval, so a
/// tier scheduler cannot promote/demote one side independently. Worker failure
/// releases every participant together and records the deterministic fallback
/// state.
#[derive(Clone, Debug)]
pub struct InteractionSchedulerV1 {
    capacity: usize,
    groups: BTreeMap<String, InteractionGroupStateV1>,
    active_by_agent: BTreeMap<u64, String>,
}

impl InteractionSchedulerV1 {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            groups: BTreeMap::new(),
            active_by_agent: BTreeMap::new(),
        }
    }

    pub fn enqueue(&mut self, request: InteractionRequestV1) -> Result<(), String> {
        request.validate().map_err(|issues| {
            issues
                .into_iter()
                .map(|issue| issue.message)
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        if self.groups.contains_key(&request.request_id) {
            return Err(format!(
                "interaction request {} is duplicated",
                request.request_id
            ));
        }
        for participant in &request.participants {
            if let Some(active) = self.active_by_agent.get(&participant.agent_id) {
                return Err(format!(
                    "participant {} is already promoted in {}",
                    participant.agent_id, active
                ));
            }
        }
        self.groups.insert(
            request.request_id.clone(),
            InteractionGroupStateV1 {
                request,
                status: InteractionGroupStatusV1::Queued,
            },
        );
        Ok(())
    }

    pub fn promote_next(&mut self) -> Option<String> {
        let active_count = self
            .groups
            .values()
            .filter(|group| group.status == InteractionGroupStatusV1::Promoted)
            .count();
        if active_count >= self.capacity {
            return None;
        }
        let request_id = self
            .groups
            .iter()
            .filter(|(_, group)| group.status == InteractionGroupStatusV1::Queued)
            .map(|(request_id, _)| request_id.clone())
            .next()?;
        let group = self.groups.get_mut(&request_id)?;
        for participant in &group.request.participants {
            self.active_by_agent
                .insert(participant.agent_id, request_id.clone());
        }
        group.status = InteractionGroupStatusV1::Promoted;
        Some(request_id)
    }

    pub fn complete(&mut self, request_id: &str) -> Option<InteractionGroupStatusV1> {
        self.release_group(request_id, InteractionGroupStatusV1::Completed)
    }

    pub fn fail(&mut self, request_id: &str, reason: &str) -> Option<InteractionGroupStatusV1> {
        let _ = reason;
        self.release_group(request_id, InteractionGroupStatusV1::Fallback)
    }

    pub fn status(&self, request_id: &str) -> Option<InteractionGroupStatusV1> {
        self.groups.get(request_id).map(|group| group.status)
    }

    pub fn active_group_for(&self, agent_id: u64) -> Option<&str> {
        self.active_by_agent.get(&agent_id).map(String::as_str)
    }

    fn release_group(
        &mut self,
        request_id: &str,
        status: InteractionGroupStatusV1,
    ) -> Option<InteractionGroupStatusV1> {
        let group = self.groups.get_mut(request_id)?;
        if group.status == InteractionGroupStatusV1::Promoted {
            for participant in &group.request.participants {
                self.active_by_agent.remove(&participant.agent_id);
            }
        }
        group.status = status;
        Some(status)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionModeV1 {
    Strict,
    Exploratory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactLabelV1 {
    None,
    Touch,
    Support,
    Impact,
    Grip,
    Forbidden,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionParticipantV1 {
    pub agent_id: u64,
    pub role: String,
    pub retarget_profile_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootSampleV1 {
    pub tick: u64,
    pub position: [f32; 3],
    pub yaw: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootConstraintV1 {
    pub agent_id: u64,
    pub samples: Vec<RootSampleV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactConstraintV1 {
    pub contact_id: String,
    pub owner_agent_id: u64,
    pub other_agent_id: u64,
    pub label: ContactLabelV1,
    pub tick_start: u64,
    pub tick_end: u64,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionProvenanceV1 {
    pub base_cache_hash: String,
    pub graph_hash: String,
    pub worker_protocol: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionBudgetsV1 {
    pub max_latency_ms: u64,
    pub max_memory_bytes: u64,
    pub max_output_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionRequestV1 {
    pub schema_version: u32,
    pub request_id: String,
    pub group_id: String,
    pub participants: Vec<InteractionParticipantV1>,
    pub tick_start: u64,
    pub tick_end: u64,
    pub seed: u64,
    pub mode: InteractionModeV1,
    pub action: String,
    pub outcome: String,
    pub root_constraints: Vec<RootConstraintV1>,
    pub contact_constraints: Vec<ContactConstraintV1>,
    pub provenance: InteractionProvenanceV1,
    pub budgets: InteractionBudgetsV1,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InteractionIssueCode {
    UnsupportedVersion,
    EmptyId,
    InvalidTickRange,
    TooFewParticipants,
    DuplicateParticipant,
    InvalidParticipant,
    MissingRootConstraint,
    DuplicateRootConstraint,
    InvalidRootSamples,
    InvalidContactConstraint,
    DuplicateContactConstraint,
    InvalidProvenance,
    InvalidBudget,
    RequestIdMismatch,
    ParticipantSetMismatch,
    MissingMotionParticipant,
    DuplicateMotionParticipant,
    InvalidMotionRoots,
    RootDiscontinuity,
    RootDeviation,
    InvalidSkeletalChannel,
    UnknownContact,
    InvalidContact,
    ForbiddenContact,
    RequiredContactMissing,
    InvalidMotionProvenance,
    MissingFallback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionIssue {
    pub code: InteractionIssueCode,
    pub message: String,
}

impl InteractionIssue {
    fn new(code: InteractionIssueCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionRootSampleV1 {
    pub tick: u64,
    pub translation: [f32; 3],
    pub yaw: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkeletalChannelV1 {
    pub joint: String,
    pub ticks: Vec<u64>,
    pub values: Vec<[f32; 4]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionParticipantV1 {
    pub agent_id: u64,
    pub root_samples: Vec<MotionRootSampleV1>,
    pub skeletal_channels: Vec<SkeletalChannelV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionContactV1 {
    pub contact_id: String,
    pub label: ContactLabelV1,
    pub owner_agent_id: u64,
    pub other_agent_id: u64,
    pub tick: u64,
    pub distance_m: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionProvenanceV1 {
    pub backend: String,
    pub model_hash: Option<String>,
    pub seed: u64,
    pub config_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackReferenceV1 {
    pub clip_set_id: String,
    pub clip_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionMotionV1 {
    pub schema_version: u32,
    pub request_id: String,
    pub participants: Vec<MotionParticipantV1>,
    pub contacts: Vec<MotionContactV1>,
    pub provenance: MotionProvenanceV1,
    pub diagnostics: Vec<String>,
    pub fallback: FallbackReferenceV1,
}

impl InteractionRequestV1 {
    pub fn validate(&self) -> Result<(), Vec<InteractionIssue>> {
        let mut issues = Vec::new();
        if self.schema_version != INTERACTION_REQUEST_SCHEMA_VERSION {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::UnsupportedVersion,
                format!("expected request schema version {INTERACTION_REQUEST_SCHEMA_VERSION}"),
            ));
        }
        if self.request_id.is_empty() || self.group_id.is_empty() {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::EmptyId,
                "request_id and group_id must be non-empty",
            ));
        }
        if self.tick_start > self.tick_end {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::InvalidTickRange,
                "tick_start must not be after tick_end",
            ));
        }
        if self.participants.len() < 2 {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::TooFewParticipants,
                "an interaction group requires at least two participants",
            ));
        }

        let mut participant_ids = BTreeSet::new();
        for participant in &self.participants {
            if participant.role.is_empty() || participant.retarget_profile_id.is_empty() {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::InvalidParticipant,
                    format!(
                        "participant {} has an empty role or retarget profile",
                        participant.agent_id
                    ),
                ));
            }
            if !participant_ids.insert(participant.agent_id) {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::DuplicateParticipant,
                    format!(
                        "participant {} appears more than once",
                        participant.agent_id
                    ),
                ));
            }
        }

        let mut roots_by_agent = BTreeMap::new();
        for root in &self.root_constraints {
            if roots_by_agent.insert(root.agent_id, root).is_some() {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::DuplicateRootConstraint,
                    format!(
                        "root constraint for agent {} appears more than once",
                        root.agent_id
                    ),
                ));
            }
            if !participant_ids.contains(&root.agent_id) {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::InvalidRootSamples,
                    format!(
                        "root constraint references non-participant {}",
                        root.agent_id
                    ),
                ));
            }
            validate_root_samples(&root.samples, self.tick_start, self.tick_end, &mut issues);
        }
        for participant_id in &participant_ids {
            if !roots_by_agent.contains_key(participant_id) {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::MissingRootConstraint,
                    format!("participant {participant_id} has no root constraint"),
                ));
            }
        }

        let mut contact_ids = BTreeSet::new();
        for contact in &self.contact_constraints {
            if !contact_ids.insert(contact.contact_id.clone()) {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::DuplicateContactConstraint,
                    format!("contact {} appears more than once", contact.contact_id),
                ));
            }
            if contact.contact_id.is_empty()
                || contact.owner_agent_id == contact.other_agent_id
                || !participant_ids.contains(&contact.owner_agent_id)
                || !participant_ids.contains(&contact.other_agent_id)
                || contact.tick_start > contact.tick_end
                || contact.tick_start < self.tick_start
                || contact.tick_end > self.tick_end
            {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::InvalidContactConstraint,
                    format!(
                        "contact {} is outside the declared interaction",
                        contact.contact_id
                    ),
                ));
            }
        }

        if !is_hash(&self.provenance.base_cache_hash)
            || !is_hash(&self.provenance.graph_hash)
            || self.provenance.worker_protocol.is_empty()
        {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::InvalidProvenance,
                "base_cache_hash and graph_hash must be 64 lowercase hex characters and worker_protocol must be non-empty",
            ));
        }
        if self.budgets.max_latency_ms == 0
            || self.budgets.max_memory_bytes == 0
            || self.budgets.max_output_bytes == 0
        {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::InvalidBudget,
                "interaction budgets must be positive",
            ));
        }

        finish_validation(issues)
    }
}

impl InteractionMotionV1 {
    pub fn validate_against(
        &self,
        request: &InteractionRequestV1,
    ) -> Result<(), Vec<InteractionIssue>> {
        let mut issues = Vec::new();
        if let Err(request_issues) = request.validate() {
            issues.extend(request_issues);
        }
        if self.schema_version != INTERACTION_MOTION_SCHEMA_VERSION {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::UnsupportedVersion,
                format!("expected motion schema version {INTERACTION_MOTION_SCHEMA_VERSION}"),
            ));
        }
        if self.request_id != request.request_id {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::RequestIdMismatch,
                format!(
                    "motion request {} does not match {}",
                    self.request_id, request.request_id
                ),
            ));
        }

        let expected_ids: BTreeSet<_> = request
            .participants
            .iter()
            .map(|item| item.agent_id)
            .collect();
        let mut actual_ids = BTreeSet::new();
        let roots_by_agent: BTreeMap<_, _> = request
            .root_constraints
            .iter()
            .map(|constraint| (constraint.agent_id, constraint))
            .collect();
        for participant in &self.participants {
            if !actual_ids.insert(participant.agent_id) {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::DuplicateMotionParticipant,
                    format!(
                        "motion participant {} appears more than once",
                        participant.agent_id
                    ),
                ));
            }
            let Some(expected_root) = roots_by_agent.get(&participant.agent_id) else {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::MissingMotionParticipant,
                    format!(
                        "motion participant {} has no request root",
                        participant.agent_id
                    ),
                ));
                continue;
            };
            validate_motion_roots(
                &participant.root_samples,
                expected_root,
                request.tick_start,
                request.tick_end,
                &mut issues,
            );
            validate_channels(&participant.skeletal_channels, &mut issues);
        }
        if actual_ids != expected_ids {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::ParticipantSetMismatch,
                "motion participants must exactly match request participants",
            ));
        }

        let constraints: BTreeMap<_, _> = request
            .contact_constraints
            .iter()
            .map(|constraint| (constraint.contact_id.as_str(), constraint))
            .collect();
        let mut observed_required = BTreeSet::new();
        let mut observed = BTreeSet::new();
        for contact in &self.contacts {
            let Some(constraint) = constraints.get(contact.contact_id.as_str()) else {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::UnknownContact,
                    format!("motion references unknown contact {}", contact.contact_id),
                ));
                continue;
            };
            if contact.owner_agent_id != constraint.owner_agent_id
                || contact.other_agent_id != constraint.other_agent_id
                || contact.label != constraint.label
                || contact.tick < constraint.tick_start
                || contact.tick > constraint.tick_end
                || !contact.distance_m.is_finite()
                || contact.distance_m < 0.0
            {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::InvalidContact,
                    format!(
                        "motion contact {} violates its declared constraint",
                        contact.contact_id
                    ),
                ));
                continue;
            }
            if !observed.insert((contact.contact_id.clone(), contact.tick)) {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::InvalidContact,
                    format!(
                        "motion contact {} is duplicated at tick {}",
                        contact.contact_id, contact.tick
                    ),
                ));
            }
            if contact.label == ContactLabelV1::Forbidden
                || (constraint.label == ContactLabelV1::Forbidden
                    && contact.distance_m <= MAX_CONTACT_DISTANCE_M)
            {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::ForbiddenContact,
                    format!("forbidden contact {} was reported", contact.contact_id),
                ));
            }
            if constraint.required
                && contact.label == constraint.label
                && contact.distance_m <= MAX_CONTACT_DISTANCE_M
            {
                observed_required.insert(contact.contact_id.clone());
            }
        }
        for constraint in &request.contact_constraints {
            if constraint.required && !observed_required.contains(&constraint.contact_id) {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::RequiredContactMissing,
                    format!(
                        "required contact {} was not observed",
                        constraint.contact_id
                    ),
                ));
            }
        }

        if self.provenance.backend.is_empty()
            || self.provenance.config_hash.is_empty()
            || (matches!(request.mode, InteractionModeV1::Strict)
                && self.provenance.seed != request.seed)
        {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::InvalidMotionProvenance,
                "motion provenance must name a backend/config and match the strict request seed",
            ));
        }
        if self.fallback.clip_set_id.is_empty()
            || self.fallback.clip_id.is_empty()
            || self.fallback.reason.is_empty()
        {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::MissingFallback,
                "motion must declare a deterministic fallback clip and reason",
            ));
        }

        finish_validation(issues)
    }
}

/// Build the model-independent R0 baseline from the request's authored roots.
pub fn deterministic_paired_clip(
    request: &InteractionRequestV1,
) -> Result<InteractionMotionV1, Vec<InteractionIssue>> {
    request.validate()?;
    let participants = request
        .root_constraints
        .iter()
        .map(|constraint| MotionParticipantV1 {
            agent_id: constraint.agent_id,
            root_samples: constraint
                .samples
                .iter()
                .map(|sample| MotionRootSampleV1 {
                    tick: sample.tick,
                    translation: sample.position,
                    yaw: sample.yaw,
                })
                .collect(),
            skeletal_channels: Vec::new(),
        })
        .collect();
    let contacts = request
        .contact_constraints
        .iter()
        .filter(|constraint| constraint.required && constraint.label != ContactLabelV1::Forbidden)
        .map(|constraint| MotionContactV1 {
            contact_id: constraint.contact_id.clone(),
            label: constraint.label,
            owner_agent_id: constraint.owner_agent_id,
            other_agent_id: constraint.other_agent_id,
            tick: constraint.tick_start + (constraint.tick_end - constraint.tick_start) / 2,
            distance_m: 0.0,
        })
        .collect();
    let motion = InteractionMotionV1 {
        schema_version: INTERACTION_MOTION_SCHEMA_VERSION,
        request_id: request.request_id.clone(),
        participants,
        contacts,
        provenance: MotionProvenanceV1 {
            backend: "authored-paired-clip".to_owned(),
            model_hash: None,
            seed: request.seed,
            config_hash: "authored-paired-clip-v1".to_owned(),
        },
        diagnostics: Vec::new(),
        fallback: FallbackReferenceV1 {
            clip_set_id: "pedestrian_basic".to_owned(),
            clip_id: "walk".to_owned(),
            reason: "deterministic paired-clip baseline".to_owned(),
        },
    };
    motion.validate_against(request).map(|_| motion)
}

fn validate_root_samples(
    samples: &[RootSampleV1],
    tick_start: u64,
    tick_end: u64,
    issues: &mut Vec<InteractionIssue>,
) {
    if samples.is_empty()
        || samples
            .first()
            .is_some_and(|sample| sample.tick != tick_start)
        || samples.last().is_some_and(|sample| sample.tick != tick_end)
    {
        issues.push(InteractionIssue::new(
            InteractionIssueCode::InvalidRootSamples,
            "root samples must cover the complete request interval",
        ));
    }
    let mut previous = None;
    for sample in samples {
        if sample.tick < tick_start
            || sample.tick > tick_end
            || previous.is_some_and(|last| sample.tick <= last)
            || sample.position.iter().any(|value| !value.is_finite())
            || !sample.yaw.is_finite()
        {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::InvalidRootSamples,
                format!("invalid or unordered root sample at tick {}", sample.tick),
            ));
        }
        previous = Some(sample.tick);
    }
}

fn validate_motion_roots(
    samples: &[MotionRootSampleV1],
    expected: &RootConstraintV1,
    tick_start: u64,
    tick_end: u64,
    issues: &mut Vec<InteractionIssue>,
) {
    if samples.is_empty()
        || samples
            .first()
            .is_some_and(|sample| sample.tick != tick_start)
        || samples.last().is_some_and(|sample| sample.tick != tick_end)
    {
        issues.push(InteractionIssue::new(
            InteractionIssueCode::InvalidMotionRoots,
            format!(
                "agent {} motion roots must cover the complete interval",
                expected.agent_id
            ),
        ));
    }
    let mut previous = None;
    for sample in samples {
        if sample.tick < tick_start
            || sample.tick > tick_end
            || previous.is_some_and(|last| sample.tick <= last)
            || sample.translation.iter().any(|value| !value.is_finite())
            || !sample.yaw.is_finite()
        {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::InvalidMotionRoots,
                format!(
                    "invalid or unordered motion root sample at tick {}",
                    sample.tick
                ),
            ));
        }
        if let Some(last_tick) = previous {
            let prior = samples
                .iter()
                .find(|candidate| candidate.tick == last_tick)
                .map(|candidate| candidate.translation)
                .unwrap_or(sample.translation);
            if distance(prior, sample.translation) > MAX_ROOT_STEP_M {
                issues.push(InteractionIssue::new(
                    InteractionIssueCode::RootDiscontinuity,
                    format!(
                        "agent {} moves too far between ticks {} and {}",
                        expected.agent_id, last_tick, sample.tick
                    ),
                ));
            }
        }
        let target = interpolate_root(expected.samples.as_slice(), sample.tick);
        if distance(target, sample.translation) > MAX_ROOT_DEVIATION_M {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::RootDeviation,
                format!(
                    "agent {} motion root deviates from its authored path at tick {}",
                    expected.agent_id, sample.tick
                ),
            ));
        }
        previous = Some(sample.tick);
    }
}

fn validate_channels(channels: &[SkeletalChannelV1], issues: &mut Vec<InteractionIssue>) {
    let mut joints = BTreeSet::new();
    for channel in channels {
        if channel.joint.is_empty()
            || channel.ticks.is_empty()
            || channel.ticks.len() != channel.values.len()
            || !joints.insert(channel.joint.clone())
            || channel.ticks.windows(2).any(|pair| pair[0] >= pair[1])
            || channel
                .values
                .iter()
                .any(|value| value.iter().any(|component| !component.is_finite()))
        {
            issues.push(InteractionIssue::new(
                InteractionIssueCode::InvalidSkeletalChannel,
                format!("invalid skeletal channel {}", channel.joint),
            ));
        }
    }
}

fn interpolate_root(samples: &[RootSampleV1], tick: u64) -> [f32; 3] {
    if let Some(exact) = samples.iter().find(|sample| sample.tick == tick) {
        return exact.position;
    }
    let Some(after_index) = samples.iter().position(|sample| sample.tick > tick) else {
        return samples.last().map_or([0.0; 3], |sample| sample.position);
    };
    if after_index == 0 {
        return samples[0].position;
    }
    let before = &samples[after_index - 1];
    let after = &samples[after_index];
    let span = (after.tick - before.tick) as f32;
    let amount = (tick - before.tick) as f32 / span;
    [
        before.position[0] + (after.position[0] - before.position[0]) * amount,
        before.position[1] + (after.position[1] - before.position[1]) * amount,
        before.position[2] + (after.position[2] - before.position[2]) * amount,
    ]
}

fn distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    let dx = left[0] - right[0];
    let dy = left[1] - right[1];
    let dz = left[2] - right[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn finish_validation(mut issues: Vec<InteractionIssue>) -> Result<(), Vec<InteractionIssue>> {
    if issues.is_empty() {
        Ok(())
    } else {
        issues.sort_by(|left, right| {
            left.code
                .cmp(&right.code)
                .then_with(|| left.message.cmp(&right.message))
        });
        Err(issues)
    }
}
