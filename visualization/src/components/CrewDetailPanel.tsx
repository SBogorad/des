import { useMemo } from 'react'
import type { VizState, FilterState } from '../types'

function formatTime(minutes: number): string {
  const d = Math.floor(minutes / 1440)
  const h = Math.floor((minutes % 1440) / 60)
  const m = minutes % 60
  return d > 0 ? `Д${d} ${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}` : `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}`
}

export function CrewDetailPanel({
  vizState,
  filter,
  selectedCrew,
  playbackMin,
}: {
  vizState: VizState
  filter: FilterState
  selectedCrew: string | null
  playbackMin: number
}) {
  const crewData = useMemo(() => {
    if (!selectedCrew) return null
    const segs = vizState.segments.filter(s => s.crewId === selectedCrew)
    return segs
  }, [vizState, selectedCrew])

  if (!crewData || crewData.length === 0) {
    return (
      <div style={styles.panel}>
        <div style={styles.empty}>
          Кликните на арку или строку таймлайна для деталей
        </div>
      </div>
    )
  }

  const travelSegs = crewData.filter(s => s.kind === 'traveling')
  const restSegs = crewData.filter(s => s.kind === 'rest_turnaround')
  const prepSegs = crewData.filter(s => s.kind === 'preparation')
  const violations = crewData.reduce((sum, s) => sum + s.violations, 0)
  const totalDistance = travelSegs.reduce((sum, s) => sum + s.distanceKm, 0)
  const totalWorkMin = crewData.reduce((sum, s) => sum + (s.endMin - s.startMin), 0)

  return (
    <div style={styles.panel}>
      <div style={styles.header}>
        <h3 style={styles.title}>{crewData[0].crewName}</h3>
        <span style={styles.badge}>{crewData[0].crewId}</span>
      </div>

      <div style={styles.statsGrid}>
        <StatCard label="Депо" value={crewData[0].depot} />
        <StatCard label="Поездок" value={travelSegs.length} />
        <StatCard label="Оборотов" value={restSegs.length} />
        <StatCard label="Нарушений" value={violations} color="#ff3355" />
        <StatCard label="Рабочее время" value={formatDuration(totalWorkMin)} />
        <StatCard label="Пробег" value={`${totalDistance.toFixed(1)} км`} />
      </div>

      <div style={styles.section}>
        <div style={styles.sectionTitle}>📋 Этапы смены</div>
        <div style={styles.timeline}>
          {crewData
            .filter(s => s.kind !== 'home_rest')
            .sort((a, b) => a.startMin - b.startMin)
            .map((seg, idx) => (
              <div key={idx} style={styles.step}>
                <div style={{
                  ...styles.stepMarker,
                  background: seg.violations > 0 ? '#ff3355' : SEGMENT_COLORS[seg.kind],
                }} />
                <div style={styles.stepContent}>
                  <div style={styles.stepTitle}>
                    {KIND_LABELS[seg.kind as keyof typeof KIND_LABELS]}
                    {seg.kind === 'traveling' && (
                      <span style={styles.route}>
                        {' '}{seg.from} → {seg.to}
                      </span>
                    )}
                  </div>
                  <div style={styles.stepMeta}>
                    <span>{formatTime(seg.startMin)} — {formatTime(seg.endMin)}</span>
                    <span style={styles.duration}>
                      {formatDuration(seg.endMin - seg.startMin)}
                    </span>
                    {seg.distanceKm > 0 && (
                      <span style={styles.distance}>{seg.distanceKm.toFixed(1)} км</span>
                    )}
                    {seg.violations > 0 && (
                      <span style={styles.violation}>⚠ {seg.violations} наруш.</span>
                    )}
                  </div>
                  {playbackMin >= seg.startMin && playbackMin <= seg.endMin && (
                    <div style={styles.activeNow}>▶ В процессе</div>
                  )}
                </div>
              </div>
            ))}
        </div>
      </div>

      <div style={styles.section}>
        <div style={styles.sectionTitle}>🚂 Локомотив</div>
        <div style={styles.locoInfo}>
          экипаж привязан к локомотиву (ID в данных симуляции)
        </div>
      </div>
    </div>
  )
}

const KIND_LABELS = {
  traveling: 'Поездка',
  rest_turnaround: 'Отдых на оборотной',
  home_rest: 'Домашний отдых',
  preparation: 'Подготовка',
} as const

const SEGMENT_COLORS = {
  traveling: '#00e87a',
  rest_turnaround: '#ffcc00',
  home_rest: '#334466',
  preparation: '#4488cc',
}

function formatDuration(min: number): string {
  const h = Math.floor(min / 60)
  const m = min % 60
  return h > 0 ? `${h}ч ${m}м` : `${m}м`
}

function StatCard({ label, value, color }: { label: string; value: string | number; color?: string }) {
  return (
    <div style={styles.statCard}>
      <div style={{ ...styles.statValue, color: color || '#00e87a' }}>{value}</div>
      <div style={styles.statLabel}>{label}</div>
    </div>
  )
}

const styles: Record<string, React.CSSProperties> = {
  panel: {
    background: '#0d1220',
    borderLeft: '1px solid #1a2a4a',
    padding: '16px 12px',
    width: 300,
    flexShrink: 0,
    overflowY: 'auto',
    display: 'flex',
    flexDirection: 'column',
    gap: 16,
  },
  empty: {
    color: '#4466aa',
    fontSize: 13,
    textAlign: 'center',
    padding: 40,
  },
  header: {
    display: 'flex',
    alignItems: 'center',
    gap: 8,
    flexWrap: 'wrap',
  },
  title: {
    margin: 0,
    fontSize: 16,
    fontWeight: 600,
    color: '#c8d8ff',
  },
  badge: {
    background: '#1a3060',
    color: '#88aaff',
    padding: '2px 8px',
    borderRadius: 4,
    fontSize: 11,
    fontFamily: 'monospace',
  },
  statsGrid: {
    display: 'grid',
    gridTemplateColumns: '1fr 1fr',
    gap: 8,
  },
  statCard: {
    background: '#0a1828',
    borderRadius: 6,
    padding: '10px 8px',
    textAlign: 'center',
    border: '1px solid #1a2a4a',
  },
  statValue: {
    fontSize: 18,
    fontWeight: 700,
  },
  statLabel: {
    fontSize: 10,
    color: '#6688aa',
    marginTop: 2,
  },
  section: {
    display: 'flex',
    flexDirection: 'column',
    gap: 8,
  },
  sectionTitle: {
    fontSize: 11,
    fontWeight: 600,
    color: '#4466aa',
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  timeline: {
    display: 'flex',
    flexDirection: 'column',
    gap: 6,
  },
  step: {
    display: 'flex',
    gap: 10,
  },
  stepMarker: {
    width: 10,
    height: 10,
    borderRadius: '50%',
    marginTop: 4,
    flexShrink: 0,
    boxShadow: '0 0 6px currentColor',
  },
  stepContent: {
    flex: 1,
    display: 'flex',
    flexDirection: 'column',
    gap: 2,
  },
  stepTitle: {
    fontSize: 12,
    fontWeight: 500,
    color: '#e8e8f0',
  },
  route: {
    fontWeight: 400,
    color: '#88aaff',
    fontSize: 11,
  },
  stepMeta: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: 8,
    fontSize: 10,
    color: '#8899bb',
  },
  duration: {
    background: '#1a2a4a',
    padding: '1px 6px',
    borderRadius: 3,
    color: '#ffcc00',
  },
  distance: {
    background: '#1a2a4a',
    padding: '1px 6px',
    borderRadius: 3,
    color: '#00e87a',
  },
  violation: {
    background: '#3a0010',
    padding: '1px 6px',
    borderRadius: 3,
    color: '#ff6677',
  },
  activeNow: {
    fontSize: 10,
    color: '#ff8844',
    fontWeight: 600,
    animation: 'pulse 1s infinite',
  },
  locoInfo: {
    fontSize: 11,
    color: '#6688aa',
    padding: 8,
    background: '#0a1828',
    borderRadius: 4,
    border: '1px solid #1a2a4a',
  },
}
