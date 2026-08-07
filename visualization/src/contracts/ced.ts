export const CED_EXPORT_SCHEMA_V1 = 'rzd-des-ced-export/v1' as const

export interface CedTrip {
  id: string
  crew_id: string
  route: string
  depot: string
  start_time: string
  end_time: string
  distance_km: number
  violations: string[]
}

export interface CedEvent {
  id: string
  trip_id: string
  shift: number
  segment: string
  factor: string
  responsible: string
  loss_hours: number
  status: string
}

export interface CedViolation {
  id: string
  trip_id: string
  kind: string
  severity: string
  description: string
  norm_reference: string
  detected_at: number
}

export interface CedFactor {
  name: string
  percent: number
  owner: string
}

export interface CedExportV1 {
  schema: typeof CED_EXPORT_SCHEMA_V1
  trips: CedTrip[]
  events: CedEvent[]
  violations: CedViolation[]
  factors: CedFactor[]
}

export function parseCedExport(input: string | unknown): CedExportV1 {
  let value: unknown = input
  if (typeof input === 'string') {
    try {
      value = JSON.parse(input)
    } catch (error) {
      throw new Error(`CED export contract error: invalid JSON (${error instanceof Error ? error.message : 'parse failure'})`)
    }
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('CED export contract error: document must be an object')
  }
  const document = value as Record<string, unknown>
  if (document.schema !== CED_EXPORT_SCHEMA_V1) {
    throw new Error(`CED export contract error: unsupported schema '${String(document.schema)}', expected ${CED_EXPORT_SCHEMA_V1}`)
  }
  for (const key of ['trips', 'events', 'violations', 'factors']) {
    if (!Array.isArray(document[key])) {
      throw new Error(`CED export contract error: ${key} must be an array`)
    }
  }
  return document as unknown as CedExportV1
}
