import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

export function buildFindings(rows) {
  return rows
  .filter(row => row.violations_count > 0)
  .map(row => {
    const critical = row.violations_count >= 2
    const station = row.to_station || row.from_station
    const eventTime = row.handover_start_time ?? row.arrival_time ?? row.departure_time

    return {
      crewId: row.crew_id,
      severity: critical ? 'critical' : 'attention',
      code: critical ? 'CREW_WORKLOAD_REVIEW' : 'TURNAROUND_REVIEW',
      title: critical ? 'Требуется проверка рабочего времени' : 'Требуется проверка оборота',
      detail: critical
        ? `В синтетическом сценарии зафиксировано нарушений: ${row.violations_count}. Проверьте рабочее время и соблюдение режима отдыха.`
        : 'В синтетическом сценарии зафиксировано нарушение оборота. Проверьте интервал отдыха перед обратным рейсом.',
      station,
      eventTime,
      recommendation: critical
        ? 'Проверить график смены и рассмотреть резервную бригаду на следующем плече.'
        : 'Проверить фактическое время передачи локомотива и достаточность отдыха.',
    }
  })
  .sort((a, b) => (a.severity === b.severity ? a.eventTime - b.eventTime : a.severity === 'critical' ? -1 : 1))
}

const isMainModule = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]

if (isMainModule) {
  const inputPath = new URL('./public/sim.json', import.meta.url)
  const outputPath = new URL('./public/findings.json', import.meta.url)
  const rows = JSON.parse(readFileSync(inputPath, 'utf8'))
  const findings = buildFindings(rows)

  writeFileSync(outputPath, `${JSON.stringify(findings, null, 2)}\n`)
  console.log(`Generated ${findings.length} findings in ${outputPath.pathname}`)
}
