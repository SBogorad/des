use crate::entities::{CrewId, LocoId, SimTime, ViolationId};
use crate::events::ViolationType;
use crate::network::SegmentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Warning,
    Critical,
    Fatal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub id: ViolationId,
    pub violation_type: ViolationType,
    pub severity: Severity,
    pub crew_id: CrewId,
    pub loco_id: Option<LocoId>,
    pub segment_id: Option<SegmentId>,
    pub detected_at: SimTime,
    pub description: String,
    pub norm_reference: String,
}

impl Default for ViolationAnalytics {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ViolationAnalytics {
    pub violations: Vec<Violation>,
    crew_index: HashMap<CrewId, Vec<usize>>,
    violation_counter: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationAnalyticsSnapshot {
    pub violations: Vec<Violation>,
    pub violation_counter: u64,
}

impl ViolationAnalytics {
    pub fn new() -> Self {
        Self {
            violations: Vec::new(),
            crew_index: HashMap::new(),
            violation_counter: 0,
        }
    }

    pub fn next_id(&mut self) -> ViolationId {
        self.violation_counter += 1;
        format!("V-{:06}", self.violation_counter)
    }

    pub fn add_violation(&mut self, v: Violation) {
        let idx = self.violations.len();
        self.crew_index
            .entry(v.crew_id.clone())
            .or_default()
            .push(idx);
        self.violations.push(v);
    }

    pub fn get_by_crew(&self, crew_id: &CrewId) -> Vec<&Violation> {
        match self.crew_index.get(crew_id) {
            Some(indices) => indices
                .iter()
                .filter_map(|&i| self.violations.get(i))
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn count_by_crew(&self, crew_id: &CrewId) -> usize {
        self.crew_index.get(crew_id).map_or(0, |v| v.len())
    }

    pub fn has_type(&self, crew_id: &CrewId, vtype: ViolationType) -> bool {
        self.get_by_crew(crew_id)
            .iter()
            .any(|v| v.violation_type == vtype)
    }

    pub fn summary(&self) -> ViolationSummary {
        let mut by_type: HashMap<ViolationType, u32> = HashMap::new();
        let mut by_severity: HashMap<String, u32> = HashMap::new();
        let mut by_crew: HashMap<CrewId, u32> = HashMap::new();
        let mut by_loco: HashMap<LocoId, u32> = HashMap::new();

        for v in &self.violations {
            *by_type.entry(v.violation_type).or_insert(0) += 1;
            *by_severity.entry(format!("{:?}", v.severity)).or_insert(0) += 1;
            *by_crew.entry(v.crew_id.clone()).or_insert(0) += 1;
            if let Some(ref lid) = v.loco_id {
                *by_loco.entry(lid.clone()).or_insert(0) += 1;
            }
        }

        ViolationSummary {
            total: self.violations.len(),
            by_type,
            by_severity,
            by_crew,
            by_loco,
        }
    }

    pub fn len(&self) -> usize {
        self.violations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn snapshot(&self) -> ViolationAnalyticsSnapshot {
        ViolationAnalyticsSnapshot {
            violations: self.violations.clone(),
            violation_counter: self.violation_counter,
        }
    }

    pub fn from_snapshot(snapshot: ViolationAnalyticsSnapshot) -> Self {
        let mut crew_index: HashMap<CrewId, Vec<usize>> = HashMap::new();
        for (idx, violation) in snapshot.violations.iter().enumerate() {
            crew_index
                .entry(violation.crew_id.clone())
                .or_default()
                .push(idx);
        }
        Self {
            violations: snapshot.violations,
            crew_index,
            violation_counter: snapshot.violation_counter,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationSummary {
    pub total: usize,
    pub by_type: HashMap<ViolationType, u32>,
    pub by_severity: HashMap<String, u32>,
    pub by_crew: HashMap<CrewId, u32>,
    pub by_loco: HashMap<LocoId, u32>,
}
