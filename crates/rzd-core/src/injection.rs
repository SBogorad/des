use serde::{Deserialize, Serialize};

use crate::entities::{FailureKind, SimTime};
use crate::events::ViolationType;
use crate::violations::Severity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionPlan {
    pub schema: String,
    pub events: Vec<InjectedEvent>,
}

impl InjectionPlan {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != "rzd-des-injection-plan/v1" {
            return Err(format!(
                "unsupported injection schema '{}', expected rzd-des-injection-plan/v1",
                self.schema
            ));
        }
        for event in &self.events {
            match event {
                InjectedEvent::WaitWindow {
                    duration_min,
                    crew_id,
                    ..
                }
                | InjectedEvent::FailureWindow {
                    duration_min,
                    crew_id,
                    ..
                }
                | InjectedEvent::VoltageSagWindow {
                    duration_min,
                    crew_id,
                    ..
                } => {
                    if *duration_min == 0 {
                        return Err(format!(
                            "injected event for crew '{}' must have duration_min > 0",
                            crew_id
                        ));
                    }
                }
                InjectedEvent::ManualViolation { .. } => {}
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InjectedEvent {
    WaitWindow {
        time: SimTime,
        crew_id: String,
        loco_id: Option<String>,
        duration_min: u64,
    },
    FailureWindow {
        time: SimTime,
        crew_id: String,
        loco_id: String,
        segment_id: String,
        failure_kind: FailureKind,
        duration_min: u64,
    },
    VoltageSagWindow {
        time: SimTime,
        crew_id: String,
        loco_id: Option<String>,
        segment_id: String,
        voltage_v: u32,
        duration_min: u64,
    },
    ManualViolation {
        time: SimTime,
        crew_id: String,
        loco_id: Option<String>,
        segment_id: Option<String>,
        violation_type: ViolationType,
        severity: Severity,
        description: String,
        norm_reference: String,
    },
}
