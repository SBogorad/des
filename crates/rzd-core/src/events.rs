use serde::{Deserialize, Serialize};
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use crate::entities::{CrewId, LocoId, SimTime, FailureKind};
use crate::network::SegmentId;

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
    VoltageSagStart { voltage_v: u32, segment_id: SegmentId },
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

impl SimEvent {
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl Ord for SimEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        other.time.cmp(&self.time)
            .then_with(|| self.priority.cmp(&other.priority))
            .then_with(|| self.id.cmp(&other.id))
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
    cancelled: std::collections::HashSet<u64>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            counter: 0,
            cancelled: std::collections::HashSet::new(),
        }
    }

    pub fn push(&mut self, mut event: SimEvent) -> u64 {
        self.counter += 1;
        event.id = self.counter;
        self.heap.push(event);
        self.counter
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
        self.heap.is_empty()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn peek(&self) -> Option<&SimEvent> {
        self.heap.peek()
    }

    pub fn clear(&mut self) {
        self.heap.clear();
        self.cancelled.clear();
        self.counter = 0;
    }
}

pub fn make_event(time: SimTime, kind: EventKind, crew_id: Option<CrewId>, loco_id: Option<LocoId>, segment_id: Option<SegmentId>, priority: u8, id: u64) -> SimEvent {
    SimEvent { time, kind, crew_id, loco_id, segment_id, priority, id }
}
