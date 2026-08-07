import type { CSSProperties } from 'react'
import type { CrewSegment, SyntheticFinding } from '../types'

const COLORS: Record<string, string> = {
  preparation: '#4e91d8',
  traveling: '#00c977',
  rest_turnaround: '#e3ad38',
  home_rest: '#38516e',
}

const LABELS: Record<string, string> = {
  preparation: 'Подготовка',
  traveling: 'Поездка',
  rest_turnaround: 'Оборот',
  home_rest: 'Отдых',
}

export function ShiftStrip({
  segments,
  playbackMin,
  finding,
  resolved,
}: {
  segments: CrewSegment[]
  playbackMin: number
  finding: SyntheticFinding | null
  resolved: boolean
}) {
  const visible = segments.filter(segment => segment.kind !== 'home_rest')
  if (visible.length === 0) return null

  const start = Math.min(...visible.map(segment => segment.startMin))
  const end = Math.max(...visible.map(segment => segment.endMin))
  const span = Math.max(1, end - start)
  const cursor = Math.max(0, Math.min(100, ((playbackMin - start) / span) * 100))

  return (
    <section style={styles.wrap} aria-label="Шкала смены">
      <div style={styles.heading}>Шкала смены</div>
      <div style={styles.track}>
        {visible.map((segment, index) => {
          const width = Math.max(5, ((segment.endMin - segment.startMin) / span) * 100)
          const isRisk = segment.violations > 0
          const color = isRisk && resolved ? '#00c977' : isRisk ? '#f14f61' : COLORS[segment.kind]
          return (
            <div key={`${segment.kind}-${index}`} style={{ ...styles.stage, width: `${width}%`, background: color }}>
              {width > 17 && <span>{LABELS[segment.kind]}</span>}
            </div>
          )
        })}
        <div style={{ ...styles.cursor, left: `${cursor}%` }} />
        {finding && (
          <div style={{ ...styles.controlPoint, left: `${Math.max(3, Math.min(97, ((finding.eventTime - start) / span) * 100))}%` }}>
            <span>{resolved ? 'Решение' : 'Контроль'}</span>
          </div>
        )}
      </div>
      <div style={styles.legend}>
        <span><i style={{ background: '#4e91d8' }} />Подготовка</span>
        <span><i style={{ background: '#00c977' }} />Поездка</span>
        <span><i style={{ background: '#e3ad38' }} />Оборот</span>
        {finding && <span><i style={{ background: resolved ? '#00c977' : '#f14f61' }} />{resolved ? 'Риск снят' : 'Точка контроля'}</span>}
      </div>
    </section>
  )
}

const styles: Record<string, CSSProperties> = {
  wrap: { padding: '12px 16px', borderBottom: '1px solid #1a2a4a' },
  heading: { marginBottom: 8, color: '#7890b3', fontSize: 10, letterSpacing: 0.7, textTransform: 'uppercase' },
  track: { height: 16, display: 'flex', position: 'relative', borderRadius: 3, overflow: 'visible', background: '#162238' },
  stage: { minWidth: 3, height: '100%', borderRight: '1px solid #0b1321', display: 'grid', placeItems: 'center', overflow: 'hidden', color: '#04120c', fontSize: 8, fontWeight: 700, whiteSpace: 'nowrap' },
  cursor: { position: 'absolute', top: -4, bottom: -4, width: 2, background: '#e8f1ff', boxShadow: '0 0 7px #78b5ff', zIndex: 2 },
  controlPoint: { position: 'absolute', top: -22, transform: 'translateX(-50%)', color: '#ffbf63', fontSize: 8, fontWeight: 700, whiteSpace: 'nowrap' },
  legend: { display: 'flex', flexWrap: 'wrap', gap: 9, marginTop: 8, color: '#7085a6', fontSize: 9 },
}
