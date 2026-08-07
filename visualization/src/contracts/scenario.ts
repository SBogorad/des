export const SCENARIO_SCHEMA_V1 = 'rzd-des-scenario/v1' as const

export interface ScenarioConfigV1 {
  crew_count: number
  loco_count: number
  seed: number
  duration_days: number
  shift_spread_min?: number
  night_shift_ratio?: number
  allowed_depot_ids?: string[]
  allowed_leg_ids?: string[]
}

export interface ScenarioDocumentV1 {
  schema: typeof SCENARIO_SCHEMA_V1
  name?: string | null
  description?: string | null
  config: ScenarioConfigV1
}

function fail(message: string): never {
  throw new Error(`scenario contract error: ${message}`)
}

export function parseScenarioDocument(input: string | unknown): ScenarioDocumentV1 {
  let value: unknown = input
  if (typeof input === 'string') {
    try {
      value = JSON.parse(input)
    } catch (error) {
      fail(`invalid JSON (${error instanceof Error ? error.message : 'parse failure'})`)
    }
  }

  if (!value || typeof value !== 'object' || Array.isArray(value)) fail('document must be an object')
  const document = value as Record<string, unknown>
  if (document.schema !== SCENARIO_SCHEMA_V1) {
    fail(`unsupported schema '${String(document.schema)}', expected ${SCENARIO_SCHEMA_V1}`)
  }
  if (!document.config || typeof document.config !== 'object' || Array.isArray(document.config)) {
    fail('config must be an object')
  }
  const config = document.config as Record<string, unknown>
  for (const key of ['crew_count', 'loco_count', 'seed', 'duration_days']) {
    if (!Number.isInteger(config[key]) || (config[key] as number) < 0) {
      fail(`config.${key} must be a non-negative integer`)
    }
  }
  if ((config.seed as number) > 0xffff_ffff) fail('config.seed exceeds u32')
  if (config.night_shift_ratio !== undefined &&
      (typeof config.night_shift_ratio !== 'number' || config.night_shift_ratio < 0 || config.night_shift_ratio > 1)) {
    fail('config.night_shift_ratio must be between 0 and 1')
  }
  for (const key of ['allowed_depot_ids', 'allowed_leg_ids']) {
    if (config[key] !== undefined &&
        (!Array.isArray(config[key]) || (config[key] as unknown[]).some(item => typeof item !== 'string'))) {
      fail(`config.${key} must be an array of strings`)
    }
  }
  return document as unknown as ScenarioDocumentV1
}
