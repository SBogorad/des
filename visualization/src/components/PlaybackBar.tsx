import type { VizState } from '../types'

function formatTime(minutes: number): string {
  const d = Math.floor(minutes / 1440)
  const h = Math.floor((minutes % 1440) / 60)
  const m = minutes % 60
  return `День ${d + 1}  ${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}`
}

export function PlaybackBar({
  vizState,
  playbackMin,
  playing,
  onChangeMin,
  onTogglePlay,
  speed,
  onSpeedChange,
  onReset,
}: {
  vizState: VizState
  playbackMin: number
  playing: boolean
  onChangeMin: (m: number) => void
  onTogglePlay: () => void
  speed: number
  onSpeedChange: (s: number) => void
  onReset: () => void
}) {
  const { minTime, maxTime } = vizState
  const pct = ((playbackMin - minTime) / (maxTime - minTime)) * 100

  return (
    <div style={styles.bar}>
      <button style={styles.btn} onClick={onReset} title="Сброс в начало">⏮</button>
      <button style={styles.btn} onClick={onTogglePlay}>
        {playing ? '⏸' : '▶'}
      </button>
      <span style={styles.label}>{formatTime(playbackMin)}</span>
      <input
        type="range"
        min={minTime}
        max={maxTime}
        value={playbackMin}
        onChange={e => onChangeMin(Number(e.target.value))}
        style={styles.slider}
      />
      <span style={styles.label}>{formatTime(maxTime)}</span>
      <div style={styles.speedControl}>
        <span style={styles.speedLabel}>Скорость:</span>
        <select
          value={speed}
          onChange={e => onSpeedChange(Number(e.target.value))}
          style={styles.speedSelect}
        >
          <option value={1}>1x</option>
          <option value={2}>2x</option>
          <option value={4}>4x</option>
          <option value={8}>8x</option>
          <option value={16}>16x</option>
          <option value={32}>32x</option>
        </select>
      </div>
    </div>
  )
}

const styles: Record<string, React.CSSProperties> = {
  bar: {
    display: 'flex', alignItems: 'center', gap: 8,
    padding: '8px 16px',
    background: '#0d1220',
    borderTop: '1px solid #1a2a4a',
    height: 44,
    flexShrink: 0,
  },
  btn: {
    background: 'transparent', border: '1px solid #1a3060',
    color: '#8899bb', width: 32, height: 28, borderRadius: 4,
    cursor: 'pointer', fontSize: 14, display: 'flex',
    alignItems: 'center', justifyContent: 'center',
  },
  label: { fontSize: 11, color: '#4466aa', whiteSpace: 'nowrap', minWidth: 90 },
  slider: { flex: 1, accentColor: '#ff8844', cursor: 'pointer' },
  speedControl: {
    display: 'flex', alignItems: 'center', gap: 6,
    marginLeft: 'auto',
  },
  speedLabel: { fontSize: 10, color: '#6688aa', whiteSpace: 'nowrap' },
  speedSelect: {
    background: '#0a1828', border: '1px solid #1a3060',
    color: '#c8d8ff', padding: '2px 8px', borderRadius: 4,
    fontSize: 10, cursor: 'pointer',
  },
}
