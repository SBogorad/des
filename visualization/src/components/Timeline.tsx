import { useMemo, useState } from 'react'
import type { VizState, FilterState, SegmentKind } from '../types'
import { SEGMENT_COLORS } from '../data/transform'

const KIND_LABELS: Record<SegmentKind, string> = {
  traveling: 'Поездка',
  rest_turnaround: 'Отдых (оборот)',
  home_rest: 'Домашний отдых',
  preparation: 'Подготовка',
}

function formatTime(minutes: number): string {
  const d = Math.floor(minutes / 1440)
  const h = Math.floor((minutes % 1440) / 60)
  const m = minutes % 60
  return d > 0 ? `Д${d} ${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}` : `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}`
}

export function Timeline({
  vizState,
  filter,
  selectedCrew,
  playbackMin,
  onSelectCrew,
}: {
  vizState: VizState
  filter: FilterState
  selectedCrew: string | null
  playbackMin: number
  onSelectCrew: (id: string | null) => void
}) {
  const ROW_H = 28
  const LABEL_W = 120
  const CHART_W = 680
  const PADDING = 8
  const [hoveredCrew, setHoveredCrew] = useState<string | null>(null)
  const [hoveredSegment, setHoveredSegment] = useState<{ crewId: string; index: number } | null>(null)

  const { minTime, maxTime } = vizState
  const timeSpan = maxTime - minTime

  const toX = (min: number) => LABEL_W + ((min - minTime) / timeSpan) * CHART_W

  const filteredCrews = useMemo(() => {
    const active = new Set(
      vizState.segments
        .filter(s => {
          if (filter.depot && s.depot !== filter.depot) return false
          if (filter.kind && s.kind !== filter.kind) return false
          if (filter.onlyViolations && s.violations === 0) return false
          return true
        })
        .map(s => s.crewId)
    )
    return vizState.crews.filter(c => active.has(c))
  }, [vizState, filter])

  const totalH = filteredCrews.length * ROW_H + PADDING * 2 + 24
  const svgW = LABEL_W + CHART_W + PADDING

  // Time axis ticks (every 6 hours = 360 min)
  const ticks = useMemo(() => {
    const out: { min: number; label: string }[] = []
    const step = 360
    const start = Math.ceil(minTime / step) * step
    for (let t = start; t <= maxTime; t += step) {
      out.push({ min: t, label: formatTime(t) })
    }
    return out
  }, [minTime, maxTime])

  const playbackX = toX(playbackMin)

  return (
    <div style={{ overflowY: 'auto', overflowX: 'hidden', height: '100%', background: '#0a0e1a' }}>
      <svg width={svgW} height={totalH} style={{ display: 'block', minWidth: svgW }}>
        {/* Grid background */}
        {ticks.map(t => (
          <line
            key={t.min}
            x1={toX(t.min)} y1={PADDING + 18} x2={toX(t.min)} y2={totalH - PADDING}
            stroke="#1a2a4a" strokeWidth={1}
          />
        ))}

        {/* Time axis */}
        {ticks.map(t => (
          <text
            key={t.min}
            x={toX(t.min)} y={PADDING + 12}
            fontSize={10} fill="#4466aa" textAnchor="middle"
          >
            {t.label}
          </text>
        ))}

        {/* Rows */}
        {filteredCrews.map((crewId, rowIdx) => {
          const y = PADDING + 24 + rowIdx * ROW_H
          const isSelected = crewId === selectedCrew
          const isHovered = crewId === hoveredCrew
          const segs = vizState.segments.filter(s => s.crewId === crewId)
          const crewName = segs[0]?.crewName ?? crewId

          return (
            <g
              key={crewId}
              onClick={() => onSelectCrew(isSelected ? null : crewId)}
              onMouseEnter={() => setHoveredCrew(crewId)}
              onMouseLeave={() => setHoveredCrew(null)}
              style={{ cursor: 'pointer' }}
            >
              {/* Row highlight */}
              {(isSelected || isHovered) && (
                <rect x={0} y={y - 2} width={svgW} height={ROW_H}
                  fill={isSelected ? "#1a3060" : "#152040"} rx={2} />
              )}

              {/* Label */}
              <text
                x={LABEL_W - 6} y={y + ROW_H / 2 + 4}
                fontSize={10} fill={isSelected ? '#ffffff' : isHovered ? '#aaccff' : '#8899bb'}
                textAnchor="end"
              >
                {crewName.length > 14 ? crewName.slice(0, 13) + '…' : crewName}
              </text>

              {/* Segments */}
              {segs.map((seg, si) => {
                if (seg.kind === 'home_rest') return null
                const x1 = toX(seg.startMin)
                const x2 = toX(seg.endMin)
                const w = Math.max(2, x2 - x1)
                const color = seg.violations > 0 ? '#ff3355' : SEGMENT_COLORS[seg.kind]
                const segH = seg.kind === 'rest_turnaround' ? ROW_H - 8 : ROW_H - 4
                const segY = y + (ROW_H - segH) / 2
                const isCurrent = playbackMin >= seg.startMin && playbackMin <= seg.endMin
                const isHoveredSeg = hoveredSegment?.crewId === crewId && hoveredSegment?.index === si

                return (
                  <g key={si} onMouseEnter={() => setHoveredSegment({ crewId, index: si })} onMouseLeave={() => setHoveredSegment(null)}>
                    <rect
                      x={x1} y={segY} width={w} height={segH}
                      fill={color} opacity={isHoveredSeg ? 1 : isCurrent ? 0.95 : 0.85} rx={3}
                      stroke={isHoveredSeg ? '#fff' : isCurrent ? '#fff' : 'none'}
                      strokeWidth={isHoveredSeg || isCurrent ? 1.5 : 0}
                      filter={isCurrent ? 'url(#glow)' : 'none'}
                    />
                    {w > 30 && (
                      <text
                        x={x1 + w / 2} y={segY + segH / 2 + 4}
                        fontSize={9} fill="#000" textAnchor="middle"
                        fontWeight="bold"
                      >
                        {KIND_LABELS[seg.kind]}
                      </text>
                    )}
                    {seg.violations > 0 && (
                      <text x={x1 + 3} y={segY + 9} fontSize={9} fill="#fff">⚠</text>
                    )}
                    {isCurrent && w > 8 && (
                      <text
                        x={x1 + w / 2} y={segY - 2}
                        fontSize={8} fill="#ff8844" textAnchor="middle"
                        fontWeight="bold"
                      >
                        ▶ LIVE
                      </text>
                    )}
                  </g>
                )
              })}
            </g>
          )
        })}

        {/* Playback cursor */}
        <line
          x1={playbackX} y1={PADDING + 18} x2={playbackX} y2={totalH - PADDING}
          stroke="#ff8844" strokeWidth={2} strokeDasharray="4 2"
        />
        <polygon
          points={`${playbackX - 5},${PADDING + 18} ${playbackX + 5},${PADDING + 18} ${playbackX},${PADDING + 26}`}
          fill="#ff8844"
        />

        <defs>
          <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="3" result="blur"/>
            <feMerge>
              <feMergeNode in="blur"/>
              <feMergeNode in="SourceGraphic"/>
            </feMerge>
          </filter>
        </defs>
      </svg>
    </div>
  )
}
