/**
 * Enrich sim.json with:
 * - clearly non-personal synthetic crew labels
 * - varied departure/appear times (spread across 3 days)
 * - arrival_time and handover data for round-trips
 */
import { readFileSync, writeFileSync } from 'fs';

const CREW_CALLSIGNS = [
  'АЛЬФА', 'БЕТА', 'ГАММА', 'ДЕЛЬТА', 'ЭХО', 'ЗЕТА', 'ИОТА', 'КАППА',
  'ЛЯМБДА', 'МЮ', 'НЮ', 'ОМЕГА', 'ПСИ', 'РО', 'СИГМА', 'ТАУ',
  'ФИ', 'ХИ', 'ИПСИЛОН', 'ЙОТА', 'КСИ', 'ЭПСИЛОН', 'ЭТА', 'ТЕТА',
];

// Mulberry32 PRNG for deterministic synthetic labels
function mulberry32(a) {
  return function() {
    a |= 0; a = a + 0x6D2B79F5 | 0;
    var t = Math.imul(a ^ a >>> 15, 1 | a);
    t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t;
    return ((t ^ t >>> 14) >>> 0) / 4294967296;
  };
}

const rand = mulberry32(42);

function pick(arr) { return arr[Math.floor(rand() * arr.length)]; }
function randRange(min, max) { return Math.floor(rand() * (max - min)) + min; }

function generateCrewLabel(crewId) {
  return `СИНТЕТИЧЕСКАЯ БРИГАДА ${pick(CREW_CALLSIGNS)} · ${crewId}`;
}

// Read network for station distances
const network = JSON.parse(readFileSync('public/network.json', 'utf8'));
const rows = JSON.parse(readFileSync('public/sim.json', 'utf8'));

console.log(`Enriching ${rows.length} TU-3 rows...`);

// Find the full sim duration (3 days = 4320 min)
const SIM_DURATION = 4320;

// Enrich each row
let enriched = rows.map((row, i) => {
  const r = { ...row };

  // Public fixtures contain an explicit synthetic label, never personnel fields.
  delete r.full_name;
  delete r.personnel_number;
  r.synthetic_label = generateCrewLabel(r.crew_id);

  // Spread departures across 3 days instead of all at time 0
  // Crew i departs at a staggered time within the first day
  const baseOffset = randRange(0, SIM_DURATION - 300); // spread across full sim
  const legTravelMin = Math.round((r.total_distance_km / Math.max(r.avg_speed_kmh, 40)) * 60);

  // Appear 20-40 min before departure
  const appearMin = baseOffset;
  const departMin = appearMin + randRange(20, 40);

  r.appear_time = appearMin;
  r.departure_time = departMin;

  // Set shift_start to appear time
  r.shift_start = appearMin;

  // arrival_time: departure + travel time
  const arriveMin = departMin + legTravelMin;
  r.arrival_time = arriveMin;

  // Handover (turnaround) - small rest after arrival before handover
  const restAfterArrival = randRange(5, 20);
  const handoverDuration = randRange(15, 45);
  r.handover_start_time = arriveMin + restAfterArrival;
  r.handover_end_time = r.handover_start_time + handoverDuration;

  // Return trip (same distance, similar time)
  const returnTravelMin = legTravelMin + randRange(-10, 10);
  const returnStart = r.handover_end_time;
  const returnEnd = returnStart + returnTravelMin;

  // Total shift should include return
  r.shift_end = returnEnd + randRange(10, 30);
  r.work_duration_min = r.shift_end - r.appear_time;

  // Fill in some extra fields for realism
  r.night_work_min = r.work_duration_min > 480 ? randRange(60, 120) : 0;
  r.day_work_min = r.work_duration_min - r.night_work_min;

  // Add med_check and acceptance times
  r.med_check_time = r.appear_time + randRange(5, 15);
  r.acceptance_start_time = r.med_check_time + randRange(3, 8);
  r.acceptance_end_time = r.acceptance_start_time + randRange(10, 25);

  // Acceptance stats
  r.acceptance_total_min = r.acceptance_end_time - r.acceptance_start_time;
  r.acceptance_issue_count = rand() > 0.7 ? randRange(1, 3) : 0;

  return r;
});

// Ensure no overlap: shift each crew's departure by at least their shift duration
// Actually the original data has non-overlapping legs, so just sort by departure
enriched.sort((a, b) => a.appear_time - b.appear_time);

// Re-number rows
enriched = enriched.map((r, i) => ({ ...r, row_number: i }));

writeFileSync('public/sim.json', JSON.stringify(enriched, null, 2));
console.log(`Wrote enriched sim.json with ${enriched.length} rows`);
console.log(`Sample: ${enriched[0].synthetic_label} depart=${enriched[0].departure_time} arrive=${enriched[0].arrival_time} shift_end=${enriched[0].shift_end}`);
console.log(`Time range: ${Math.min(...enriched.map(r=>r.appear_time))} - ${Math.max(...enriched.map(r=>r.shift_end))}`);
