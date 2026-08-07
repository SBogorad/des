export interface Station {
  id: string
  name: string
  kind: 'MainDepot' | 'Waypoint' | 'Junction' | 'Turnaround' | 'Terminal'
  region: string
  coords: [number, number]
  depot_id: string | null
}

export interface Leg {
  id: string
  name: string
  segments: string[]
  total_km: number
}

export interface NetworkData {
  stations: Record<string, Station>
  segments: Record<string, { id: string; from: string; to: string; distance_km: number }>
  legs?: Record<string, Leg>
}

export interface Tu3Row {
  row_number: number
  crew_id: string
  loco_id: string
  depot_name: string
  loco_series: string
  from_station: string
  to_station: string
  shift_start: number
  shift_end: number | null
  appear_time: number
  departure_time: number
  arrival_time: number | null
  handover_start_time: number | null
  handover_end_time: number | null
  total_distance_km: number
  violations_count: number
  work_duration_min: number
  // optional synthetic identity fields
  full_name?: string
  personnel_number?: number
}

export type SegmentKind = 'traveling' | 'rest_turnaround' | 'home_rest' | 'preparation'

export interface CrewSegment {
  crewId: string
  crewName: string
  depot: string
  locomotiveId: string
  locomotiveSeries: string
  from: string
  to: string
  startMin: number
  endMin: number
  kind: SegmentKind
  violations: number
  distanceKm: number
  waypoints: [number, number, number][] | null
}

export type FindingSeverity = 'attention' | 'critical'

export interface SyntheticFinding {
  crewId: string
  severity: FindingSeverity
  code: string
  title: string
  detail: string
  station: string
  eventTime: number
  recommendation: string
}

export interface VizState {
  segments: CrewSegment[]
  network: NetworkData
  minTime: number
  maxTime: number
  crews: string[]
  findings: SyntheticFinding[]
}

export type FilterState = {
  depot: string | null
  kind: SegmentKind | null
  onlyViolations: boolean
}
