use crate::network::{DepotId, LegId, SegmentId, StationId};
use serde::{Deserialize, Serialize};

pub type CrewId = String;
pub type LocoId = String;
pub type SimTime = u64;
pub type ViolationId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrewQualification {
    Assistant,
    Junior,
    Senior,
    Instructor,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrewState {
    AtHome,
    Appearing,
    MedCheck,
    PreTripCheck,
    Acceptance,
    Waiting,
    Working,
    Reserve,
    RestingAway,
    RestingHome,
    Finished,
}

impl CrewState {
    pub fn can_transition_to(&self, next: &CrewState) -> bool {
        matches!(
            (self, next),
            (CrewState::AtHome, CrewState::Appearing)
                | (CrewState::Appearing, CrewState::MedCheck)
                | (CrewState::MedCheck, CrewState::PreTripCheck)
                | (CrewState::PreTripCheck, CrewState::Acceptance)
                | (CrewState::Acceptance, CrewState::Waiting)
                | (CrewState::Waiting, CrewState::Working)
                | (CrewState::Working, CrewState::Waiting)
                | (CrewState::Working, CrewState::Reserve)
                | (CrewState::Working, CrewState::RestingAway)
                | (CrewState::Working, CrewState::Finished)
                | (CrewState::Waiting, CrewState::Finished)
                | (CrewState::Reserve, CrewState::Finished)
                | (CrewState::RestingAway, CrewState::Working)
                | (CrewState::RestingAway, CrewState::Finished)
                | (CrewState::Finished, CrewState::RestingHome)
                | (CrewState::RestingHome, CrewState::AtHome)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crew {
    pub id: CrewId,
    pub depot_id: DepotId,
    pub state: CrewState,
    pub shift_work_min: u32,
    pub shift_start: Option<SimTime>,
    pub cumulative_work_min: u32,
    pub last_home_rest_end: Option<SimTime>,
    pub night_shifts_in_row: u8,
    pub total_shifts: u32,
    pub current_leg: Option<LegId>,
    pub current_loco: Option<LocoId>,
    pub position: StationId,
    pub violations: Vec<ViolationId>,
    pub acceptance_issues: Vec<AcceptanceIssue>,
    pub route_segments: Vec<SegmentId>,
    pub route_index: usize,
    // Tracking fields
    pub overtime_8h_flagged: bool,
    pub total_distance_km: f32,
    pub in_voltage_sag: bool,
    // Synthetic identity fields (populated by scenario generator)
    #[serde(default)]
    pub personnel_number: u32,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub assistant_personnel_number: u32,
    #[serde(default)]
    pub assistant_full_name: String,
    #[serde(default)]
    pub qualification: Option<CrewQualification>,
}

impl Crew {
    pub fn new(id: CrewId, depot_id: DepotId, position: StationId) -> Self {
        Self {
            id,
            depot_id,
            state: CrewState::AtHome,
            shift_work_min: 0,
            shift_start: None,
            cumulative_work_min: 0,
            last_home_rest_end: None,
            night_shifts_in_row: 0,
            total_shifts: 0,
            current_leg: None,
            current_loco: None,
            position,
            violations: Vec::new(),
            acceptance_issues: Vec::new(),
            route_segments: Vec::new(),
            route_index: 0,
            overtime_8h_flagged: false,
            total_distance_km: 0.0,
            in_voltage_sag: false,
            personnel_number: 0,
            full_name: String::new(),
            assistant_personnel_number: 0,
            assistant_full_name: String::new(),
            qualification: None,
        }
    }

    pub fn is_night_shift(&self) -> bool {
        if let Some(start) = self.shift_start {
            let hour = (start % 1440) / 60;
            !(6..22).contains(&hour)
        } else {
            false
        }
    }

    pub fn work_remaining_min(&self, max_8h: u32) -> i32 {
        max_8h as i32 - self.shift_work_min as i32
    }

    pub fn set_state(&mut self, new_state: CrewState) -> bool {
        if self.state.can_transition_to(&new_state) {
            self.state = new_state;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceIssue {
    pub kind: AcceptanceIssueKind,
    pub delay_min: u32,
    pub resolved: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AcceptanceIssueKind {
    PreviousCrewDelayed,
    KlubUFault,
    RadioFault,
    MissingDocuments,
    MaintenanceNotDone,
    VoltageLowAtDepot,
    SealBroken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Locomotive {
    pub id: LocoId,
    pub series: String,
    pub loco_kind: LocoKind,
    pub sections: u8,
    pub depot_id: DepotId,
    pub state: LocoState,
    pub position: StationId,
    pub mileage_km: f32,
    pub hours_since_to: f32,
    pub failures_total: u32,
    // Synthetic identity fields (populated by scenario generator)
    #[serde(default)]
    pub inventory_number: u32,
    #[serde(default = "default_manufacture_year")]
    pub manufacture_year: u16,
}

fn default_manufacture_year() -> u16 {
    2015
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocoKind {
    Electric,
    Diesel,
    Dual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocoState {
    Available,
    InUse,
    Maintenance { until: SimTime },
    Failed,
}

impl LocoState {
    pub fn can_transition_to(&self, new_state: &LocoState) -> bool {
        matches!(
            (self, new_state),
            (LocoState::Available, LocoState::InUse)
                | (LocoState::Available, LocoState::Maintenance { .. })
                | (LocoState::InUse, LocoState::Available)
                | (LocoState::InUse, LocoState::Failed)
                | (LocoState::Failed, LocoState::Available)
                | (LocoState::Maintenance { .. }, LocoState::Available)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FailureKind {
    TractionMotorOverheat,
    PantographFault,
    BrakeSystemFault,
    KlubUFault,
    RadioFault,
    VoltageLow,
    PowerSupplyFault,
    WheelDefect,
    EngineOilLeak,
    CoolingSystemFault,
}

impl FailureKind {
    pub fn severity(&self) -> FailureSeverity {
        match self {
            FailureKind::TractionMotorOverheat => FailureSeverity::Critical,
            FailureKind::PantographFault => FailureSeverity::Critical,
            FailureKind::BrakeSystemFault => FailureSeverity::Critical,
            FailureKind::KlubUFault => FailureSeverity::Warning,
            FailureKind::RadioFault => FailureSeverity::Warning,
            FailureKind::VoltageLow => FailureSeverity::Warning,
            FailureKind::PowerSupplyFault => FailureSeverity::Warning,
            FailureKind::WheelDefect => FailureSeverity::Critical,
            FailureKind::EngineOilLeak => FailureSeverity::Warning,
            FailureKind::CoolingSystemFault => FailureSeverity::Warning,
        }
    }

    pub fn all() -> &'static [FailureKind] {
        &[
            FailureKind::TractionMotorOverheat,
            FailureKind::PantographFault,
            FailureKind::BrakeSystemFault,
            FailureKind::KlubUFault,
            FailureKind::RadioFault,
            FailureKind::PowerSupplyFault,
            FailureKind::WheelDefect,
            FailureKind::EngineOilLeak,
            FailureKind::CoolingSystemFault,
            FailureKind::VoltageLow,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureSeverity {
    Warning,
    Critical,
}
