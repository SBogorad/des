use serde::{Deserialize, Serialize};
use crate::entities::{LocoId, SimTime};
use crate::network::SegmentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EtcLevel {
    Level0,
    Level1,
    Level2,
    Level3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EtcMode {
    Nominal,
    StaffResponsible,
    Isolation,
    Reversing,
    Trip,
    NonLeading,
    Standby,
    Sleep,
    SystemFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementAuthority {
    pub segment_id: SegmentId,
    pub end_position_km: f32,
    pub speed_limit_kmh: f32,
    pub valid_until: SimTime,
    pub level: EtcLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainDataMessage {
    pub train_id: String,
    pub loco_id: LocoId,
    pub length_m: f32,
    pub gross_mass_t: f32,
    pub max_speed_kmh: f32,
    pub braking_category: BrakingCategory,
    pub level: EtcLevel,
    pub mode: EtcMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrakingCategory {
    G1,  // Goods train category 1
    G2,  // Goods train category 2
    P,   // Passenger
    PP,  // High-speed passenger
}

impl BrakingCategory {
    pub fn deceleration_mps2(self) -> f32 {
        match self {
            Self::G1 => 0.5,
            Self::G2 => 0.65,
            Self::P => 0.8,
            Self::PP => 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionReport {
    pub train_id: String,
    pub position_km: f32,
    pub speed_kmh: f32,
    pub segment_id: SegmentId,
    pub timestamp: SimTime,
    pub level: EtcLevel,
    pub mode: EtcMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionData {
    pub permitted_speed_kmh: f32,
    pub intervention_speed_kmh: f32,
    pub warning_speed_kmh: f32,
    pub target_distance_km: f32,
    pub target_speed_kmh: f32,
    pub release_speed_kmh: f32,
    pub location_based_service_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaliseGroup {
    pub id: String,
    pub segment_id: SegmentId,
    pub position_km: f32,
    pub group_type: BaliseType,
    pub linked: bool,
    pub data: BaliseData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaliseType {
    Fixed,
    Configurable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaliseData {
    pub linkings: Vec<BaliseLinking>,
    pub level_crossing_free: bool,
    pub train_data: Option<TrainDataMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaliseLinking {
    pub next_group_id: String,
    pub distance_m: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadioMessageKind {
    MovementAuthority,
    EmergencyStop,
    GeneralMessage,
    RadioInfill,
    SessionEstablished,
    SessionTerminated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioMessage {
    pub kind: RadioMessageKind,
    pub from: String,
    pub to: String,
    pub timestamp: SimTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErtmsConfig {
    pub level: EtcLevel,
    pub country_code: u32,
    pub network_id: u32,
    pub nid_radio: u32,
    pub onboard_telephone_number: String,
    pub min_safe_area_m: f32,
    pub reaction_time_s: f32,
}

impl Default for ErtmsConfig {
    fn default() -> Self {
        Self {
            level: EtcLevel::Level1,
            country_code: 643, // Russia
            network_id: 1,
            nid_radio: 1000,
            onboard_telephone_number: "+7-800-000-0000".into(),
            min_safe_area_m: 100.0,
            reaction_time_s: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErtmsState {
    pub level: EtcLevel,
    pub mode: EtcMode,
    pub movement_authority: Option<MovementAuthority>,
    pub supervision: Option<SupervisionData>,
    pub last_position: Option<PositionReport>,
    pub radio_messages: Vec<RadioMessage>,
    pub trip_distance_km: f32,
}

impl ErtmsState {
    pub fn new(config: &ErtmsConfig) -> Self {
        Self {
            level: config.level,
            mode: EtcMode::Standby,
            movement_authority: None,
            supervision: None,
            last_position: None,
            radio_messages: Vec::new(),
            trip_distance_km: 0.0,
        }
    }

    pub fn activate(&mut self) {
        self.mode = EtcMode::Nominal;
    }

    pub fn deactivate(&mut self) {
        self.mode = EtcMode::Sleep;
    }

    pub fn set_trip(&mut self) {
        self.mode = EtcMode::Trip;
    }

    pub fn set_staff_responsible(&mut self) {
        self.mode = EtcMode::StaffResponsible;
    }

    pub fn request_movement_authority(&mut self, segment_id: SegmentId, end_km: f32, speed_kmh: f32, valid_until: SimTime, now: SimTime) {
        self.movement_authority = Some(MovementAuthority {
            segment_id,
            end_position_km: end_km,
            speed_limit_kmh: speed_kmh,
            valid_until,
            level: self.level,
        });
        self.radio_messages.push(RadioMessage {
            kind: RadioMessageKind::MovementAuthority,
            from: "RBC".into(),
            to: "Train".into(),
            timestamp: now,
        });
    }

    pub fn revoke_movement_authority(&mut self, now: SimTime) {
        self.movement_authority = None;
        self.radio_messages.push(RadioMessage {
            kind: RadioMessageKind::EmergencyStop,
            from: "RBC".into(),
            to: "Train".into(),
            timestamp: now,
        });
    }

    pub fn update_supervision(&mut self, current_speed_kmh: f32, distance_to_end_km: f32) {
        let ma = match &self.movement_authority {
            Some(ma) => ma,
            None => return,
        };

        let permitted = ma.speed_limit_kmh;
        let warning = permitted * 1.05;
        let intervention = permitted * 1.10;
        let release = permitted * 0.75;

        let _reaction_dist = current_speed_kmh / 3.6 * ErtmsConfig::default().reaction_time_s / 1000.0;

        self.supervision = Some(SupervisionData {
            permitted_speed_kmh: permitted,
            intervention_speed_kmh: intervention,
            warning_speed_kmh: warning,
            target_distance_km: distance_to_end_km,
            target_speed_kmh: 0.0,
            release_speed_kmh: release,
            location_based_service_active: self.level == EtcLevel::Level2 || self.level == EtcLevel::Level3,
        });
    }

    pub fn update_position(&mut self, position_km: f32, speed_kmh: f32, segment_id: SegmentId, now: SimTime) {
        let prev = self.last_position.as_ref().map(|p| p.position_km).unwrap_or(position_km);
        self.trip_distance_km += (position_km - prev).abs();

        self.last_position = Some(PositionReport {
            train_id: String::new(),
            position_km,
            speed_kmh,
            segment_id,
            timestamp: now,
            level: self.level,
            mode: self.mode,
        });

        if let Some(ma) = &self.movement_authority {
            let dist_to_end = ma.end_position_km - position_km;
            self.update_supervision(speed_kmh, dist_to_end);
        }
    }

    pub fn is_over_speed(&self) -> bool {
        if let (Some(sup), Some(pos)) = (&self.supervision, &self.last_position) {
            pos.speed_kmh > sup.permitted_speed_kmh
        } else {
            false
        }
    }

    pub fn is_intervening(&self) -> bool {
        if let (Some(sup), Some(pos)) = (&self.supervision, &self.last_position) {
            pos.speed_kmh > sup.intervention_speed_kmh
        } else {
            false
        }
    }

    pub fn check_ma_validity(&self, now: SimTime) -> bool {
        match &self.movement_authority {
            Some(ma) => ma.valid_until > now,
            None => false,
        }
    }

    pub fn radio_message_count(&self) -> usize {
        self.radio_messages.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErtmsViolation {
    pub kind: ErtmsViolationKind,
    pub timestamp: SimTime,
    pub position_km: f32,
    pub speed_kmh: f32,
    pub permitted_kmh: f32,
    pub severity: ErtmsSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErtmsViolationKind {
    Overspeed,
    Intervention,
    MaExpired,
    MaExceeded,
    LevelMismatch,
    ModeViolation,
    BaliseError,
    RadioTimeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErtmsSeverity {
    Warning,
    Emergency,
    Critical,
}

pub struct ErtmsComplianceChecker {
    pub config: ErtmsConfig,
    pub violations: Vec<ErtmsViolation>,
}

impl ErtmsComplianceChecker {
    pub fn new(config: ErtmsConfig) -> Self {
        Self {
            config,
            violations: Vec::new(),
        }
    }

    pub fn check(&mut self, state: &ErtmsState, now: SimTime) {
        if state.mode == EtcMode::Nominal {
            if let Some(pos) = &state.last_position {
                if let Some(sup) = &state.supervision {
                    if pos.speed_kmh > sup.permitted_speed_kmh {
                        self.violations.push(ErtmsViolation {
                            kind: if pos.speed_kmh > sup.intervention_speed_kmh {
                                ErtmsViolationKind::Intervention
                            } else {
                                ErtmsViolationKind::Overspeed
                            },
                            timestamp: now,
                            position_km: pos.position_km,
                            speed_kmh: pos.speed_kmh,
                            permitted_kmh: sup.permitted_speed_kmh,
                            severity: if pos.speed_kmh > sup.intervention_speed_kmh {
                                ErtmsSeverity::Emergency
                            } else {
                                ErtmsSeverity::Warning
                            },
                        });
                    }
                }

                if let Some(ma) = &state.movement_authority {
                    if pos.position_km > ma.end_position_km {
                        self.violations.push(ErtmsViolation {
                            kind: ErtmsViolationKind::MaExceeded,
                            timestamp: now,
                            position_km: pos.position_km,
                            speed_kmh: pos.speed_kmh,
                            permitted_kmh: ma.speed_limit_kmh,
                            severity: ErtmsSeverity::Critical,
                        });
                    }
                }
            }

            if !state.check_ma_validity(now) && state.movement_authority.is_some() {
                self.violations.push(ErtmsViolation {
                    kind: ErtmsViolationKind::MaExpired,
                    timestamp: now,
                    position_km: state.last_position.as_ref().map_or(0.0, |p| p.position_km),
                    speed_kmh: state.last_position.as_ref().map_or(0.0, |p| p.speed_kmh),
                    permitted_kmh: 0.0,
                    severity: ErtmsSeverity::Emergency,
                });
            }
        }
    }

    pub fn violations(&self) -> &[ErtmsViolation] {
        &self.violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ErtmsConfig {
        ErtmsConfig::default()
    }

    #[test]
    fn test_ertms_state_lifecycle() {
        let config = test_config();
        let mut state = ErtmsState::new(&config);
        assert_eq!(state.mode, EtcMode::Standby);

        state.activate();
        assert_eq!(state.mode, EtcMode::Nominal);

        state.set_trip();
        assert_eq!(state.mode, EtcMode::Trip);

        state.set_staff_responsible();
        assert_eq!(state.mode, EtcMode::StaffResponsible);

        state.deactivate();
        assert_eq!(state.mode, EtcMode::Sleep);
    }

    #[test]
    fn test_movement_authority() {
        let config = test_config();
        let mut state = ErtmsState::new(&config);
        state.activate();

        state.request_movement_authority("SEG-001".into(), 50.0, 120.0, 600, 0);
        assert!(state.movement_authority.is_some());
        assert_eq!(state.radio_message_count(), 1);

        state.revoke_movement_authority(300);
        assert!(state.movement_authority.is_none());
        assert_eq!(state.radio_message_count(), 2);
    }

    #[test]
    fn test_supervision_update() {
        let config = test_config();
        let mut state = ErtmsState::new(&config);
        state.activate();
        state.request_movement_authority("SEG-001".into(), 100.0, 120.0, 600, 0);

        state.update_position(0.0, 100.0, "SEG-001".into(), 0);
        assert!(state.supervision.is_some());

        let sup = state.supervision.as_ref().unwrap();
        assert!((sup.permitted_speed_kmh - 120.0).abs() < 0.1);
        assert!((sup.warning_speed_kmh - 126.0).abs() < 0.1);
        assert!((sup.intervention_speed_kmh - 132.0).abs() < 0.1);
    }

    #[test]
    fn test_over_speed_detection() {
        let config = test_config();
        let mut state = ErtmsState::new(&config);
        state.activate();
        state.request_movement_authority("SEG-001".into(), 100.0, 120.0, 600, 0);

        state.update_position(0.0, 110.0, "SEG-001".into(), 0);
        assert!(!state.is_over_speed());

        state.update_position(10.0, 125.0, "SEG-001".into(), 60);
        assert!(state.is_over_speed());
        assert!(!state.is_intervening());

        state.update_position(20.0, 135.0, "SEG-001".into(), 120);
        assert!(state.is_intervening());
    }

    #[test]
    fn test_compliance_checker() {
        let config = test_config();
        let mut state = ErtmsState::new(&config);
        state.activate();
        state.request_movement_authority("SEG-001".into(), 100.0, 120.0, 600, 0);

        let mut checker = ErtmsComplianceChecker::new(config);

        state.update_position(0.0, 110.0, "SEG-001".into(), 0);
        checker.check(&state, 0);
        assert!(checker.violations().is_empty());

        state.update_position(10.0, 135.0, "SEG-001".into(), 60);
        checker.check(&state, 60);
        assert!(!checker.violations().is_empty());

        let v = &checker.violations()[0];
        assert_eq!(v.kind, ErtmsViolationKind::Intervention);
        assert_eq!(v.severity, ErtmsSeverity::Emergency);
    }

    #[test]
    fn test_ma_expiry() {
        let config = test_config();
        let mut state = ErtmsState::new(&config);
        state.activate();
        state.request_movement_authority("SEG-001".into(), 100.0, 120.0, 300, 0);

        state.update_position(0.0, 100.0, "SEG-001".into(), 0);

        let mut checker = ErtmsComplianceChecker::new(config);
        checker.check(&state, 400);
        assert!(!checker.violations().is_empty());
        assert_eq!(checker.violations()[0].kind, ErtmsViolationKind::MaExpired);
    }

    #[test]
    fn test_braking_category() {
        assert!((BrakingCategory::G1.deceleration_mps2() - 0.5).abs() < 0.01);
        assert!((BrakingCategory::PP.deceleration_mps2() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_trip_distance_tracking() {
        let config = test_config();
        let mut state = ErtmsState::new(&config);
        state.activate();
        state.request_movement_authority("SEG-001".into(), 100.0, 120.0, 600, 0);

        state.update_position(0.0, 100.0, "SEG-001".into(), 0);
        assert!((state.trip_distance_km - 0.0).abs() < 0.1);

        state.update_position(50.0, 100.0, "SEG-001".into(), 300);
        assert!((state.trip_distance_km - 50.0).abs() < 0.1);

        state.update_position(80.0, 100.0, "SEG-001".into(), 480);
        assert!((state.trip_distance_km - 80.0).abs() < 0.1);
    }
}
