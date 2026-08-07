import { CED_EXPORT_SCHEMA_V1, parseCedExport, type CedExportV1 } from './ced'
import { SCENARIO_SCHEMA_V1, parseScenarioDocument, type ScenarioDocumentV1 } from './scenario'

export const EVENT_TRACE_SCHEMA_V1 = 'rzd-des-event-trace/v1' as const
export const REPLAY_BUNDLE_SCHEMA_V1 = 'rzd-des-replay-bundle/v1' as const

export interface EventTraceV1 {
  schema: typeof EVENT_TRACE_SCHEMA_V1
  until_minutes: number
  events: Array<{
    time: number
    kind: unknown
    crew_id: string | null
    loco_id: string | null
    segment_id: string | null
    priority: number
    id: number
  }>
}

export interface ReplayBundleV1 {
  schema: typeof REPLAY_BUNDLE_SCHEMA_V1
  engine: { name: string; version: string }
  replay: {
    seed: number
    duration_days: number
    until_minutes: number
    warmup_minutes: number
    scenario: ScenarioDocumentV1
  }
  network: unknown
  results: {
    stats: unknown
    violation_summary: unknown
    ced: CedExportV1
    tu3_row_count: number
  }
}

function parseObject(input: string | unknown, label: string): Record<string, unknown> {
  let value: unknown = input
  if (typeof input === 'string') {
    try {
      value = JSON.parse(input)
    } catch (error) {
      throw new Error(`${label} contract error: invalid JSON (${error instanceof Error ? error.message : 'parse failure'})`)
    }
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} contract error: document must be an object`)
  }
  return value as Record<string, unknown>
}

export function parseEventTrace(input: string | unknown): EventTraceV1 {
  const document = parseObject(input, 'event trace')
  if (document.schema !== EVENT_TRACE_SCHEMA_V1) {
    throw new Error(`event trace contract error: unsupported schema '${String(document.schema)}', expected ${EVENT_TRACE_SCHEMA_V1}`)
  }
  if (!Number.isInteger(document.until_minutes) || !Array.isArray(document.events)) {
    throw new Error('event trace contract error: until_minutes and events are required')
  }
  return document as unknown as EventTraceV1
}

export function parseReplayBundle(input: string | unknown): ReplayBundleV1 {
  const document = parseObject(input, 'replay bundle')
  if (document.schema !== REPLAY_BUNDLE_SCHEMA_V1) {
    throw new Error(`replay bundle contract error: unsupported schema '${String(document.schema)}', expected ${REPLAY_BUNDLE_SCHEMA_V1}`)
  }
  const replay = parseObject(document.replay, 'replay bundle.replay')
  const results = parseObject(document.results, 'replay bundle.results')
  const scenario = parseScenarioDocument(replay.scenario)
  const ced = parseCedExport(results.ced)
  if (scenario.schema !== SCENARIO_SCHEMA_V1 || ced.schema !== CED_EXPORT_SCHEMA_V1) {
    throw new Error('replay bundle contract error: nested schema mismatch')
  }
  if (!Number.isInteger(results.tu3_row_count) || (results.tu3_row_count as number) < 0) {
    throw new Error('replay bundle contract error: tu3_row_count must be a non-negative integer')
  }
  return { ...document, replay: { ...replay, scenario }, results: { ...results, ced } } as unknown as ReplayBundleV1
}
