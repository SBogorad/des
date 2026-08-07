import type { FilterState, SegmentKind, VizState } from '../types'
import { SEGMENT_COLORS } from '../data/transform'

const KIND_OPTIONS: { value: SegmentKind; label: string }[] = [
  { value: 'traveling', label: '🚂 Поездка' },
  { value: 'rest_turnaround', label: '💤 Отдых (оборот)' },
  { value: 'preparation', label: '📋 Подготовка' },
]

export function FilterPanel({
  vizState,
  filter,
  onChange,
}: {
  vizState: VizState
  filter: FilterState
  onChange: (f: FilterState) => void
}) {
  const depots = [...new Set(vizState.segments.map(s => s.depot))].sort()

  const stats = {
    traveling: vizState.segments.filter(s => s.kind === 'traveling').length,
    rest: vizState.segments.filter(s => s.kind === 'rest_turnaround').length,
    violations: vizState.segments.filter(s => s.violations > 0).length,
  }

  return (
    <div style={styles.panel}>
      <div style={styles.title}>Цифровой двойник — Оборот бригад</div>
      <div style={styles.subtitle}>Симуляция · {vizState.crews.length} бригад</div>

      <div style={styles.statRow}>
        <Stat label="Поездок" value={stats.traveling} color="#00e87a" />
        <Stat label="Оборотов" value={stats.rest} color="#ffcc00" />
        <Stat label="Нарушений" value={stats.violations} color="#ff3355" />
      </div>

      <div style={styles.section}>Депо</div>
      <button
        style={{ ...styles.chip, ...(filter.depot === null ? styles.chipActive : {}) }}
        onClick={() => onChange({ ...filter, depot: null })}
      >Все</button>
      {depots.map(d => (
        <button
          key={d}
          style={{ ...styles.chip, ...(filter.depot === d ? styles.chipActive : {}) }}
          onClick={() => onChange({ ...filter, depot: filter.depot === d ? null : d })}
        >
          {d.replace('DEP-', 'Д').replace('SD-', 'ОД-').replace('TD-', 'Т-')}
        </button>
      ))}

      <div style={styles.section}>Состояние</div>
      {KIND_OPTIONS.map(opt => (
        <button
          key={opt.value}
          style={{
            ...styles.chip,
            ...(filter.kind === opt.value ? styles.chipActive : {}),
            borderColor: SEGMENT_COLORS[opt.value],
          }}
          onClick={() => onChange({ ...filter, kind: filter.kind === opt.value ? null : opt.value })}
        >
          {opt.label}
        </button>
      ))}

      <div style={styles.section}>Флаги</div>
      <button
        style={{ ...styles.chip, ...(filter.onlyViolations ? styles.chipDanger : {}) }}
        onClick={() => onChange({ ...filter, onlyViolations: !filter.onlyViolations })}
      >
        ⚠️ Только нарушения
      </button>

      <div style={{ marginTop: 24, fontSize: 10, color: '#334466' }}>
        <div style={{ marginBottom: 4 }}>Легенда:</div>
        {KIND_OPTIONS.map(o => (
          <div key={o.value} style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 3 }}>
            <div style={{ width: 12, height: 12, borderRadius: 3, background: SEGMENT_COLORS[o.value] }} />
            <span style={{ color: '#6688aa' }}>{o.label}</span>
          </div>
        ))}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 3 }}>
          <div style={{ width: 12, height: 12, borderRadius: 3, background: '#ff3355' }} />
          <span style={{ color: '#6688aa' }}>🔴 Нарушение</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <div style={{ width: 12, height: 3, background: '#ff8844' }} />
          <span style={{ color: '#6688aa' }}>Позиция (время)</span>
        </div>
      </div>
    </div>
  )
}

function Stat({ label, value, color }: { label: string; value: number; color: string }) {
  return (
    <div style={{ textAlign: 'center' }}>
      <div style={{ fontSize: 22, fontWeight: 700, color }}>{value}</div>
      <div style={{ fontSize: 10, color: '#6688aa' }}>{label}</div>
    </div>
  )
}

const styles: Record<string, React.CSSProperties> = {
  panel: {
    background: '#0d1220',
    borderRight: '1px solid #1a2a4a',
    padding: '16px 12px',
    width: 180,
    flexShrink: 0,
    overflowY: 'auto',
    display: 'flex',
    flexDirection: 'column',
    gap: 4,
  },
  title: { fontSize: 13, fontWeight: 700, color: '#c8d8ff', marginBottom: 2 },
  subtitle: { fontSize: 10, color: '#4466aa', marginBottom: 8 },
  statRow: { display: 'flex', justifyContent: 'space-between', background: '#0a1828', borderRadius: 6, padding: '8px 4px', marginBottom: 8 },
  section: { fontSize: 10, color: '#4466aa', marginTop: 10, marginBottom: 4, textTransform: 'uppercase', letterSpacing: 1 },
  chip: {
    display: 'block', width: '100%', textAlign: 'left',
    background: 'transparent', border: '1px solid #1a3060',
    color: '#8899bb', padding: '4px 8px', borderRadius: 4,
    cursor: 'pointer', fontSize: 11, marginBottom: 3,
    transition: 'all 0.15s',
  },
  chipActive: { background: '#1a3060', color: '#ffffff', borderColor: '#4488ff' },
  chipDanger: { background: '#3a0010', color: '#ff6677', borderColor: '#ff3355' },
}
