use crate::entities::{CrewId, FailureKind, LocoId, SimTime};
use crate::network::SegmentId;
use crate::violations::Severity;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ViolationType {
    Overtime8h,
    Overtime12h,
    ShortRest,
    ShortHomeRest,
    NightOverload,
    AcceptanceDelay,
    VoltageSagShift,
    MonthlyOvertime,
    LineChangeForced,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventKind {
    // Shift Lifecycle
    ShiftAppear,
    MedCheckStart,
    MedCheckEnd,
    PreTripBriefingEnd,
    AcceptanceStart,
    AcceptanceEnd,
    DepartureCheckpoint,

    // Waiting
    WaitStart,
    WaitEnd,

    // Movement
    ArriveStation,
    PassWaypoint,

    // Failures
    FailureOccurred(FailureKind),
    FailureResolved,
    LineChangeStart,

    // Voltage
    VoltageSagStart {
        voltage_v: u32,
        segment_id: SegmentId,
    },
    VoltageSagEnd,

    // Reserve
    ReserveStart,
    ReserveEnd,

    // Turnaround
    TurnaroundRestStart,
    TurnaroundRestEnd,
    HandoverStart,
    HandoverEnd,
    ArrivalCheckpoint,
    ShiftEnd,

    // Home Rest
    HomeRestStart,
    HomeRestEnd,

    // Violations
    ViolationDetected(ViolationType),
    InjectedViolation {
        violation_type: ViolationType,
        severity: Severity,
        description: String,
        norm_reference: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimEvent {
    pub time: SimTime,
    pub kind: EventKind,
    pub crew_id: Option<CrewId>,
    pub loco_id: Option<LocoId>,
    pub segment_id: Option<SegmentId>,
    pub priority: u8,
    pub id: u64,
}

impl Default for SimEvent {
    fn default() -> Self {
        Self::new()
    }
}
impl SimEvent {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn new() -> Self {
        SimEvent {
            time: 0,
            kind: EventKind::ShiftAppear,
            crew_id: None,
            loco_id: None,
            segment_id: None,
            priority: 0,
            id: 0,
        }
    }

    /// Reset all fields to default for pool reuse
    pub fn reset(&mut self) {
        self.time = 0;
        self.kind = EventKind::ShiftAppear;
        self.crew_id = None;
        self.loco_id = None;
        self.segment_id = None;
        self.priority = 0;
        self.id = 0;
    }
}

impl Ord for SimEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .time
            .cmp(&self.time)
            .then_with(|| self.priority.cmp(&other.priority))
            .then_with(|| other.id.cmp(&self.id))
    }
}

impl PartialOrd for SimEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EventQueue {
    heap: BinaryHeap<SimEvent>,
    counter: u64,
    cancelled: HashSet<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventQueueSnapshot {
    pub pending_events: Vec<SimEvent>,
    pub counter: u64,
    pub cancelled: Vec<u64>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            counter: 0,
            cancelled: HashSet::new(),
        }
    }

    pub fn push(&mut self, mut event: SimEvent) -> u64 {
        self.counter += 1;
        event.id = self.counter;
        self.heap.push(event);
        self.counter
    }

    pub fn push_existing(&mut self, event: SimEvent) {
        self.heap.push(event);
    }

    pub fn cancel(&mut self, id: u64) {
        self.cancelled.insert(id);
    }

    pub fn pop(&mut self) -> Option<SimEvent> {
        loop {
            let event = self.heap.pop()?;
            if self.cancelled.remove(&event.id) {
                continue;
            }
            return Some(event);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.heap.len() <= self.cancelled.len()
    }

    pub fn len(&self) -> usize {
        self.heap.len() - self.cancelled.len()
    }

    /// Peek at the next event. May return a cancelled event — use pop() for guaranteed non-cancelled.
    pub fn peek(&self) -> Option<&SimEvent> {
        self.heap.peek()
    }

    pub fn clear(&mut self) {
        self.heap.clear();
        self.cancelled.clear();
        self.counter = 0;
    }

    pub fn snapshot(&self) -> EventQueueSnapshot {
        let mut cancelled: Vec<_> = self.cancelled.iter().copied().collect();
        cancelled.sort_unstable();
        EventQueueSnapshot {
            pending_events: self.heap.clone().into_vec(),
            counter: self.counter,
            cancelled,
        }
    }

    pub fn from_snapshot(snapshot: EventQueueSnapshot) -> Self {
        Self {
            heap: BinaryHeap::from(snapshot.pending_events),
            counter: snapshot.counter,
            cancelled: snapshot.cancelled.into_iter().collect(),
        }
    }
}

pub fn make_event(
    time: SimTime,
    kind: EventKind,
    crew_id: Option<CrewId>,
    loco_id: Option<LocoId>,
    segment_id: Option<SegmentId>,
    priority: u8,
    id: u64,
) -> SimEvent {
    SimEvent {
        time,
        kind,
        crew_id,
        loco_id,
        segment_id,
        priority,
        id,
    }
}
