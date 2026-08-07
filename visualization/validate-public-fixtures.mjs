import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { buildFindings } from './generate-findings.mjs'

const publicDir = new URL('./public/', import.meta.url)
const readJson = name => JSON.parse(readFileSync(new URL(name, publicDir), 'utf8'))
const failures = []

function check(condition, message) {
  if (!condition) failures.push(message)
}

function isNonNegativeInteger(value) {
  return Number.isInteger(value) && value >= 0
}

const network = readJson('network.json')
const sim = readJson('sim.json')
const findingsText = readFileSync(new URL('findings.json', publicDir), 'utf8')
const findings = JSON.parse(findingsText)

check(Array.isArray(sim) && sim.length > 0, 'sim.json must contain at least one synthetic row')
check(Array.isArray(findings), 'findings.json must be an array')

const stations = network.stations ?? {}
const segments = network.segments ?? {}
const legs = network.legs ?? {}
const stationNames = new Set()

for (const [stationId, station] of Object.entries(stations)) {
  check(/^ST-\d{3}$/.test(stationId), `invalid station key: ${stationId}`)
  check(station.id === stationId, `station id does not match key: ${stationId}`)
  check(typeof station.name === 'string' && station.name.length > 0, `station name missing: ${stationId}`)
  stationNames.add(station.name)
}

for (const [segmentId, segment] of Object.entries(segments)) {
  check(/^SEG-\d{3}$/.test(segmentId), `invalid segment key: ${segmentId}`)
  check(segment.id === segmentId, `segment id does not match key: ${segmentId}`)
  check(stations[segment.from], `segment ${segmentId} has unknown from station: ${segment.from}`)
  check(stations[segment.to], `segment ${segmentId} has unknown to station: ${segment.to}`)
  check(Number.isFinite(segment.distance_km) && segment.distance_km > 0, `segment ${segmentId} has invalid distance`)
}

for (const [legId, leg] of Object.entries(legs)) {
  check(/^LEG-\d{2}$/.test(legId), `invalid leg key: ${legId}`)
  check(leg.id === legId, `leg id does not match key: ${legId}`)
  check(Array.isArray(leg.segments) && leg.segments.length > 0, `leg ${legId} has no segments`)
  for (const segmentId of leg.segments ?? []) {
    check(segments[segmentId], `leg ${legId} references unknown segment: ${segmentId}`)
  }
}

const crewIds = new Set()
for (const row of sim) {
  const rowPrefix = `row ${row.row_number}`
  check(/^CREW-\d{4}$/.test(row.crew_id), `${rowPrefix} has invalid crew_id`)
  check(/^LOCO-\d{4}$/.test(row.loco_id), `${rowPrefix} has invalid loco_id`)
  check(!('full_name' in row) && !('personnel_number' in row), `${rowPrefix} contains prohibited identity fields`)
  check(
    typeof row.synthetic_label === 'string' && row.synthetic_label.startsWith('СИНТЕТИЧЕСКАЯ БРИГАДА '),
    `${rowPrefix} must have an explicit synthetic_label`,
  )
  check(legs[row.leg_id], `${rowPrefix} references unknown leg_id: ${row.leg_id}`)
  check(stationNames.has(row.from_station), `${rowPrefix} has unknown from_station: ${row.from_station}`)
  check(stationNames.has(row.to_station), `${rowPrefix} has unknown to_station: ${row.to_station}`)

  for (const field of ['shift_start', 'appear_time', 'departure_time', 'work_duration_min']) {
    check(isNonNegativeInteger(row[field]), `${rowPrefix} has invalid ${field}`)
  }
  for (const field of ['shift_end', 'arrival_time', 'handover_start_time', 'handover_end_time']) {
    check(row[field] === null || isNonNegativeInteger(row[field]), `${rowPrefix} has invalid ${field}`)
  }

  check(row.shift_start === row.appear_time, `${rowPrefix} shift_start must equal appear_time`)
  check(row.appear_time <= row.departure_time, `${rowPrefix} departure precedes appearance`)
  if (row.arrival_time !== null) check(row.departure_time <= row.arrival_time, `${rowPrefix} arrival precedes departure`)
  if (row.handover_start_time !== null && row.arrival_time !== null) {
    check(row.arrival_time <= row.handover_start_time, `${rowPrefix} handover starts before arrival`)
  }
  if (row.handover_end_time !== null && row.handover_start_time !== null) {
    check(row.handover_start_time <= row.handover_end_time, `${rowPrefix} handover end precedes start`)
  }
  if (row.shift_end !== null) {
    check(row.shift_end >= row.departure_time, `${rowPrefix} shift ends before departure`)
    check(row.work_duration_min === row.shift_end - row.appear_time, `${rowPrefix} work duration is inconsistent`)
  }
  crewIds.add(row.crew_id)
}

for (const finding of findings) {
  check(crewIds.has(finding.crewId), `finding has unknown crewId: ${finding.crewId}`)
  check(stationNames.has(finding.station), `finding has unknown station: ${finding.station}`)
  check(['attention', 'critical'].includes(finding.severity), `finding has invalid severity: ${finding.severity}`)
  check(isNonNegativeInteger(finding.eventTime), `finding has invalid eventTime for ${finding.crewId}`)
}

const expectedFindings = `${JSON.stringify(buildFindings(sim), null, 2)}\n`
check(findingsText === expectedFindings, 'findings.json is not the deterministic output of generate-findings.mjs')

if (failures.length > 0) {
  console.error('Public fixture validation failed:')
  for (const failure of failures) console.error(`- ${failure}`)
  process.exit(1)
}

console.log(`Validated ${sim.length} synthetic rows, ${Object.keys(stations).length} stations, and ${findings.length} findings.`)
