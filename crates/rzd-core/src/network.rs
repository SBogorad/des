use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;
use crate::norms::RegionNormsConfig;

pub type StationId = String;
pub type SegmentId = String;
pub type RegionId = String;
pub type DepotId = String;
pub type RoadId = String;
pub type LegId = String;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Missing station: {0}")]
    MissingStation(String),
    #[error("Missing segment: {0}")]
    MissingSegment(String),
    #[error("Segment references unknown station: {0} -> {1}")]
    OrphanSegment(String, String),
    #[error("Leg references unknown segment: {0}")]
    OrphanLeg(String),
    #[error("Orphan station {0}: not referenced by any road or leg")]
    OrphanStation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub id: StationId,
    pub name: String,
    pub kind: StationKind,
    pub region: RegionId,
    pub coords: Option<(f32, f32)>,
    pub depot_id: Option<DepotId>,
    pub voltage_kv: VoltageKind,
    pub electrification: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StationKind {
    MainDepot,
    Turnaround,
    Waypoint,
    Junction,
    Terminal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VoltageKind {
    DC3000,
    AC25000,
    Diesel,
    Dual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: SegmentId,
    pub from: StationId,
    pub to: StationId,
    pub distance_km: f32,
    pub base_speed_kmh: f32,
    pub gradient_factor: f32,
    pub failure_base_rate: f32,
    pub voltage_sag_prob: f32,
    pub single_track: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leg {
    pub id: LegId,
    pub name: String,
    pub from_depot: DepotId,
    pub to_depot: DepotId,
    pub segments: Vec<SegmentId>,
    pub total_km: f32,
    pub road_id: RoadId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Road {
    pub id: RoadId,
    pub name: String,
    pub stations: Vec<StationId>,
    pub depots: Vec<DepotId>,
    pub legs: Vec<LegId>,
    pub region_multiplier: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub stations: HashMap<StationId, Station>,
    pub segments: HashMap<SegmentId, Segment>,
    pub legs: HashMap<LegId, Leg>,
    pub roads: HashMap<RoadId, Road>,
    #[serde(skip)]
    adjacency: HashMap<StationId, Vec<(StationId, SegmentId)>>,
    #[serde(default)]
    pub region_norms: RegionNormsConfig,
}

impl Network {
    pub fn new(
        stations: HashMap<StationId, Station>,
        segments: HashMap<SegmentId, Segment>,
        legs: HashMap<LegId, Leg>,
        roads: HashMap<RoadId, Road>,
    ) -> Self {
        let mut network = Self {
            stations,
            segments,
            legs,
            roads,
            adjacency: HashMap::new(),
            region_norms: RegionNormsConfig::new(),
        };
        network.build_adjacency();
        network
    }

    pub fn with_region_norms(mut self, region_norms: RegionNormsConfig) -> Self {
        self.region_norms = region_norms;
        self
    }

    pub fn region_for_segment(&self, segment_id: &str) -> Option<&str> {
        let seg = self.segments.get(segment_id)?;
        self.stations.get(&seg.from).map(|s| s.region.as_str())
    }

    pub fn from_json(json: &str) -> Result<Self, NetworkError> {
        let raw: RawNetwork = serde_json::from_str(json)?;

        let mut network = Self {
            stations: raw.stations,
            segments: raw.segments,
            legs: raw.legs,
            roads: raw.roads,
            adjacency: HashMap::new(),
            region_norms: raw.region_norms.unwrap_or_default(),
        };

        network.build_adjacency();

        let errors = network.validate();
        if !errors.is_empty() {
            let msg = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
            return Err(NetworkError::MissingStation(msg));
        }

        Ok(network)
    }

    fn build_adjacency(&mut self) {
        self.adjacency.clear();
        for (seg_id, seg) in &self.segments {
            self.adjacency
                .entry(seg.from.clone())
                .or_default()
                .push((seg.to.clone(), seg_id.clone()));
            self.adjacency
                .entry(seg.to.clone())
                .or_default()
                .push((seg.from.clone(), seg_id.clone()));
        }
    }

    pub fn get_neighbors(&self, station_id: &StationId) -> Vec<(&StationId, &SegmentId)> {
        self.adjacency
            .get(station_id)
            .map(|v| v.iter().map(|(s, sid)| (s, sid)).collect())
            .unwrap_or_default()
    }

    pub fn get_segment(&self, segment_id: &SegmentId) -> Option<&Segment> {
        self.segments.get(segment_id)
    }

    pub fn get_station(&self, station_id: &StationId) -> Option<&Station> {
        self.stations.get(station_id)
    }

    pub fn get_leg_segments(&self, leg_id: &LegId) -> Option<Vec<&Segment>> {
        self.legs.get(leg_id).map(|leg| {
            leg.segments
                .iter()
                .filter_map(|sid| self.segments.get(sid))
                .collect()
        })
    }

    pub fn segment_travel_time_min(&self, segment_id: &SegmentId) -> Option<f32> {
        let seg = self.segments.get(segment_id)?;
        let base_time_min = (seg.distance_km / seg.base_speed_kmh) * 60.0;
        Some(base_time_min * seg.gradient_factor)
    }

    /// BFS shortest path (by number of hops)
    pub fn shortest_path(&self, from: &StationId, to: &StationId) -> Option<Vec<SegmentId>> {
        if from == to {
            return Some(Vec::new());
        }

        let mut visited: HashSet<&StationId> = HashSet::new();
        let mut queue: VecDeque<(&StationId, Vec<SegmentId>)> = VecDeque::new();

        visited.insert(from);
        if let Some(neighbors) = self.adjacency.get(from) {
            for (neighbor, seg_id) in neighbors {
                if !visited.contains(neighbor) {
                    queue.push_back((neighbor, vec![seg_id.clone()]));
                    visited.insert(neighbor);
                }
            }
        }

        while let Some((current, path)) = queue.pop_front() {
            if current == to {
                return Some(path);
            }
            if let Some(neighbors) = self.adjacency.get(current) {
                for (neighbor, seg_id) in neighbors {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor);
                        let mut new_path = path.clone();
                        new_path.push(seg_id.clone());
                        queue.push_back((neighbor, new_path));
                    }
                }
            }
        }

        None
    }

    /// Dijkstra shortest path (by weighted travel time in minutes)
    pub fn shortest_path_dijkstra(&self, from: &StationId, to: &StationId) -> Option<(Vec<SegmentId>, f32)> {
        use std::collections::BinaryHeap;

        if from == to {
            return Some((Vec::new(), 0.0));
        }

        let station_ids: Vec<&StationId> = self.stations.keys().collect();
        let id_to_idx: HashMap<&StationId, usize> = station_ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();

        let start_idx = *id_to_idx.get(from)?;
        let end_idx = *id_to_idx.get(to)?;

        let mut dist: Vec<f32> = vec![f32::INFINITY; station_ids.len()];
        let mut prev: Vec<Option<(usize, String)>> = vec![None; station_ids.len()];

        // Min-heap: (cost, station) — reversed comparison for min-heap
        let mut heap: BinaryHeap<(u64, usize)> = BinaryHeap::new();

        dist[start_idx] = 0.0;
        // Pack float bits into u64 for ordering (NaN-safe)
        heap.push((0.0f32.to_bits() as u64, start_idx));

        while let Some((cost_bits, u)) = heap.pop() {
            let cost = f32::from_bits(cost_bits as u32);
            if u == end_idx {
                break;
            }
            if cost > dist[u] {
                continue;
            }

            let u_id = station_ids[u];
            if let Some(neighbors) = self.adjacency.get(u_id) {
                for (v_id, seg_id) in neighbors {
                    if let Some(&v_idx) = id_to_idx.get(v_id) {
                        let travel_time = self.segment_travel_time_min(seg_id).unwrap_or(0.0);
                        let next_cost = cost + travel_time;
                        if next_cost < dist[v_idx] {
                            dist[v_idx] = next_cost;
                            prev[v_idx] = Some((u, seg_id.clone()));
                            heap.push((next_cost.to_bits() as u64, v_idx));
                        }
                    }
                }
            }
        }

        if dist[end_idx] == f32::INFINITY {
            return None;
        }

        let mut path = Vec::new();
        let mut current = end_idx;
        while let Some((parent, ref seg_id)) = prev[current] {
            path.push(seg_id.clone());
            current = parent;
        }
        path.reverse();

        Some((path, dist[end_idx]))
    }

    /// Get the ordered list of station IDs along a path (segment sequence)
    pub fn path_stations(&self, segments: &[SegmentId]) -> Vec<StationId> {
        let mut result = Vec::new();
        for sid in segments {
            if let Some(seg) = self.segments.get(sid) {
                if result.is_empty() {
                    result.push(seg.from.clone());
                }
                result.push(seg.to.clone());
            }
        }
        result
    }

    /// Validate network integrity
    pub fn validate(&self) -> Vec<NetworkError> {
        let mut errors = Vec::new();
        let mut referenced_stations: HashSet<StationId> = HashSet::new();

        for seg in self.segments.values() {
            if !self.stations.contains_key(&seg.from) {
                errors.push(NetworkError::OrphanSegment(seg.from.clone(), seg.to.clone()));
            } else {
                referenced_stations.insert(seg.from.clone());
            }
            if !self.stations.contains_key(&seg.to) {
                errors.push(NetworkError::OrphanSegment(seg.from.clone(), seg.to.clone()));
            } else {
                referenced_stations.insert(seg.to.clone());
            }
        }

        for leg in self.legs.values() {
            for sid in &leg.segments {
                if !self.segments.contains_key(sid) {
                    errors.push(NetworkError::OrphanLeg(sid.clone()));
                }
            }
            let stations = road_stations_from_leg(&self.segments, leg);
            for sid in &stations {
                referenced_stations.insert(sid.clone());
            }
        }

        for road in self.roads.values() {
            for sid in &road.stations {
                referenced_stations.insert(sid.clone());
            }
            for did in &road.depots {
                referenced_stations.insert(did.clone());
            }
        }

        for station_id in self.stations.keys() {
            if !referenced_stations.contains(station_id) {
                errors.push(NetworkError::OrphanStation(station_id.clone()));
            }
        }

        errors
    }
}

fn road_stations_from_leg(segments: &HashMap<SegmentId, Segment>, leg: &Leg) -> Vec<StationId> {
    let mut stations = Vec::new();
    for sid in &leg.segments {
        if let Some(seg) = segments.get(sid) {
            if stations.is_empty() {
                stations.push(seg.from.clone());
            }
            stations.push(seg.to.clone());
        }
    }
    stations
}

#[derive(Deserialize)]
struct RawNetwork {
    stations: HashMap<StationId, Station>,
    segments: HashMap<SegmentId, Segment>,
    legs: HashMap<LegId, Leg>,
    roads: HashMap<RoadId, Road>,
    region_norms: Option<RegionNormsConfig>,
}
