use crate::entities::CrewState;
use crate::events::EventKind;
use std::fmt;

#[derive(Debug)]
pub enum EngineError {
    MissingCrewId,
    MissingSegmentId,
    CrewNotFound(String),
    SegmentNotFound(String),
    LocoNotFound(String),
    RouteEmpty(String),
    InvalidInjection(String),
    StateTransition {
        crew_id: String,
        from: CrewState,
        to: CrewState,
    },
    UnhandledEvent {
        kind: EventKind,
        crew_id: Option<String>,
    },
    PriorityQueueInternal(String),
    SimulationOverflow {
        max_events: u64,
        current: u64,
    },
    Desync {
        source: String,
        expected: u64,
        actual: u64,
    },
    CheckpointCorrupted {
        reason: String,
        field: Option<String>,
    },
    ExternalDependencyFailed {
        source: String,
        detail: String,
    },
    BatchExportCancelled {
        exported: u64,
        total: u64,
    },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::MissingCrewId => write!(f, "event missing crew_id"),
            EngineError::MissingSegmentId => write!(f, "event missing segment_id"),
            EngineError::CrewNotFound(id) => write!(f, "crew not found: {}", id),
            EngineError::SegmentNotFound(id) => write!(f, "segment not found: {}", id),
            EngineError::LocoNotFound(id) => write!(f, "locomotive not found: {}", id),
            EngineError::RouteEmpty(id) => write!(f, "crew {} has empty route", id),
            EngineError::InvalidInjection(message) => write!(f, "invalid injection: {}", message),
            EngineError::StateTransition { crew_id, from, to } => {
                write!(
                    f,
                    "invalid state transition for {}: {:?} -> {:?}",
                    crew_id, from, to
                )
            }
            EngineError::UnhandledEvent { kind, crew_id } => {
                write!(f, "unhandled event {:?} for crew {:?}", kind, crew_id)
            }
            EngineError::PriorityQueueInternal(msg) => {
                write!(f, "priority queue internal: {}", msg)
            }
            EngineError::SimulationOverflow {
                max_events,
                current,
            } => {
                write!(
                    f,
                    "simulation event limit reached: {} (max: {})",
                    current, max_events
                )
            }
            EngineError::Desync {
                source,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "simulation desync at {}: expected event {}, got {}",
                    source, expected, actual
                )
            }
            EngineError::CheckpointCorrupted { reason, ref field } => {
                if let Some(field_name) = field {
                    write!(f, "checkpoint corrupted at '{}': {}", field_name, reason)
                } else {
                    write!(f, "checkpoint corrupted: {}", reason)
                }
            }
            EngineError::ExternalDependencyFailed { source, detail } => {
                write!(f, "external dependency '{}' failed: {}", source, detail)
            }
            EngineError::BatchExportCancelled { exported, total } => {
                write!(
                    f,
                    "batch export cancelled after {}/{} events",
                    exported, total
                )
            }
        }
    }
}

impl std::error::Error for EngineError {}

impl EngineError {
    pub fn pq(msg: &str) -> Self {
        EngineError::PriorityQueueInternal(msg.to_string())
    }

    pub fn overflow(max_events: u64, current: u64) -> Self {
        EngineError::SimulationOverflow {
            max_events,
            current,
        }
    }

    pub fn desync(source: &str, expected: u64, actual: u64) -> Self {
        EngineError::Desync {
            source: source.to_string(),
            expected,
            actual,
        }
    }

    pub fn checkpoint(reason: &str, field: Option<&str>) -> Self {
        EngineError::CheckpointCorrupted {
            reason: reason.to_string(),
            field: field.map(|s| s.to_string()),
        }
    }

    pub fn export_cancelled(exported: u64, total: u64) -> Self {
        EngineError::BatchExportCancelled { exported, total }
    }
}
