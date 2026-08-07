import type { Tu3Row, CrewSegment, VizState, NetworkData } from '../types'

// Map station coordinates into a 3D scene space that preserves geographic proportions.
// Original coords: X 50–1700 (width 1650), Y 60–220 (height 160).
// We want the map to look like a real diagonal railway, not a flat ribbon.
// Scene X range: -12 to +12 (width 24). Scene Z range: -4.5 to +4.5 (depth 9).
// That gives a ~2.7:1 ratio which makes the diagonal drift clearly visible.
const SRC_X_MIN = 50, SRC_X_MAX = 1700  // 1650 units wide
const SRC_Y_MIN = 60, SRC_Y_MAX = 220   // 160 units tall
const SCENE_W = 24   // scene X: -12 to +12
const SCENE_D = 9    // scene Z: -4.5 to +4.5

export function stationPos(stationName: string, network: NetworkData): [number, number, number] {
  const station = Object.values(network.stations).find(s => s.name === stationName)
  if (!station) return [0, 0, 0]
  const [cx, cy] = station.coords
  const x = ((cx - SRC_X_MIN) / (SRC_X_MAX - SRC_X_MIN)) * SCENE_W - SCENE_W / 2
  const z = ((cy - SRC_Y_MIN) / (SRC_Y_MAX - SRC_Y_MIN)) * SCENE_D - SCENE_D / 2
  return [x, 0, z]
}

// Build the ordered list of 3D waypoints a crew travels through on a named leg.
// Falls back to a straight line if the leg isn't found.
export function routeWaypoints(
  fromStation: string,
  toStation: string,
  network: NetworkData,
): [number, number, number][] {
  // Find the leg that connects these two depots by name match
  const legKey = `${fromStation} - ${toStation}`
  const reverseLegKey = `${toStation} - ${fromStation}`

  const legs = network.legs ?? {}
  let leg = Object.values(legs).find(
    l => l.name === legKey || l.name === reverseLegKey
  )

  // Also try matching by from/to depot station names
  if (!leg) {
    leg = Object.values(legs).find(l => {
      const segs = l.segments
        .map((sid: string) => network.segments[sid])
        .filter(Boolean)
      if (!segs.length) return false
      const first = network.stations[segs[0].from]?.name
      const last = network.stations[segs[segs.length - 1].to]?.name
      return (
        (first === fromStation && last === toStation) ||
        (first === toStation && last === fromStation)
      )
    })
  }

  if (!leg) {
    // No leg found — just two endpoints
    return [stationPos(fromStation, network), stationPos(toStation, network)]
  }

  const segIds: string[] = leg.segments
  const segList = segIds.map((sid: string) => network.segments[sid]).filter(Boolean)

  // Determine direction (forward or reverse)
  const firstSeg = segList[0]
  const firstStation = network.stations[firstSeg?.from]?.name
  const isForward = firstStation === fromStation

  const stationIds = isForward
    ? [segList[0].from, ...segList.map((s: { to: string }) => s.to)]
    : [segList[segList.length - 1].to, ...segList.reverse().map((s: { from: string }) => s.from)]

  const points = stationIds
    .map((id: string) => network.stations[id])
    .filter(Boolean)
    .map((st: { name: string }) => stationPos(st.name, network))

  return points.length >= 2 ? points : [stationPos(fromStation, network), stationPos(toStation, network)]
}

export function buildVizState(rows: Tu3Row[], network: NetworkData): VizState {
  const segments: CrewSegment[] = []
  const crewLastEnd: Record<string, number> = {}

  const sorted = [...rows].sort((a, b) => a.appear_time - b.appear_time)

  for (const row of sorted) {
    const name = row.full_name ?? row.crew_id

    // preparation: appear → departure (same station)
    if (row.departure_time > row.appear_time) {
      segments.push({
        crewId: row.crew_id,
        crewName: name,
        depot: row.depot_name,
        locomotiveId: row.loco_id,
        locomotiveSeries: row.loco_series || 'Электровоз',
        from: row.from_station,
        to: row.from_station,
        startMin: row.appear_time,
        endMin: row.departure_time,
        kind: 'preparation',
        violations: 0,
        distanceKm: 0,
        waypoints: null,
      })
    }

    // outbound trip: departure → arrival
    if (row.arrival_time !== null) {
      segments.push({
        crewId: row.crew_id,
        crewName: name,
        depot: row.depot_name,
        locomotiveId: row.loco_id,
        locomotiveSeries: row.loco_series || 'Электровоз',
        from: row.from_station,
        to: row.to_station,
        startMin: row.departure_time,
        endMin: row.arrival_time,
        kind: 'traveling',
        violations: row.violations_count,
        distanceKm: row.total_distance_km,
        waypoints: routeWaypoints(row.from_station, row.to_station, network),
      })

      // rest at turnaround
      const restEnd = row.handover_start_time ?? row.handover_end_time
      if (restEnd !== null && restEnd > row.arrival_time) {
        segments.push({
          crewId: row.crew_id,
          crewName: name,
          depot: row.depot_name,
          locomotiveId: row.loco_id,
          locomotiveSeries: row.loco_series || 'Электровоз',
          from: row.to_station,
          to: row.to_station,
          startMin: row.arrival_time,
          endMin: restEnd,
          kind: 'rest_turnaround',
          violations: 0,
          distanceKm: 0,
          waypoints: null,
        })
      }

      // return trip: handover_end → shift_end
      const returnStart = row.handover_end_time ?? restEnd
      const returnEnd = row.shift_end
      if (returnStart !== null && returnEnd !== null && returnEnd > returnStart) {
        segments.push({
          crewId: row.crew_id,
          crewName: name,
          depot: row.depot_name,
          locomotiveId: row.loco_id,
          locomotiveSeries: row.loco_series || 'Электровоз',
          from: row.to_station,
          to: row.from_station,
          startMin: returnStart,
          endMin: returnEnd,
          kind: 'traveling',
          violations: 0,
          distanceKm: row.total_distance_km,
          waypoints: routeWaypoints(row.to_station, row.from_station, network),
        })
      }
    } else {
      // single leg with no recorded arrival
      const end = row.shift_end ?? row.departure_time + row.work_duration_min
      segments.push({
        crewId: row.crew_id,
        crewName: name,
        depot: row.depot_name,
        locomotiveId: row.loco_id,
        locomotiveSeries: row.loco_series || 'Электровоз',
        from: row.from_station,
        to: row.to_station,
        startMin: row.departure_time,
        endMin: end,
        kind: 'traveling',
        violations: row.violations_count,
        distanceKm: row.total_distance_km,
        waypoints: routeWaypoints(row.from_station, row.to_station, network),
      })
    }

    // home rest gap between consecutive shifts
    const prevEnd = crewLastEnd[row.crew_id]
    if (prevEnd !== undefined && row.appear_time > prevEnd + 60) {
      segments.push({
        crewId: row.crew_id,
        crewName: name,
        depot: row.depot_name,
        locomotiveId: row.loco_id,
        locomotiveSeries: row.loco_series || 'Электровоз',
        from: row.from_station,
        to: row.from_station,
        startMin: prevEnd,
        endMin: row.appear_time,
        kind: 'home_rest',
        violations: 0,
        distanceKm: 0,
        waypoints: null,
      })
    }
    crewLastEnd[row.crew_id] = row.shift_end ?? row.departure_time + row.work_duration_min
  }

  const allTimes = segments.flatMap(s => [s.startMin, s.endMin])
  const minTime = Math.min(...allTimes)
  const maxTime = Math.max(...allTimes)
  const crews = [...new Set(sorted.map(r => r.crew_id))]

  return { segments, network, minTime, maxTime, crews, findings: [] }
}

export const SEGMENT_COLORS: Record<string, string> = {
  traveling: '#00e87a',
  rest_turnaround: '#ffcc00',
  home_rest: '#334466',
  preparation: '#4488cc',
}

export const CREW_LABELS = [
  '#f94144','#f3722c','#f8961e','#f9c74f','#90be6d',
  '#43aa8b','#4d908e','#577590','#277da1','#a8dadc',
]
