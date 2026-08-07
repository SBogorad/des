import { useState, useEffect, useRef, useCallback, useMemo } from 'react'
import type { Tu3Row, NetworkData, VizState, FilterState, CrewSegment, SyntheticFinding } from './types'
import { buildVizState } from './data/transform'
import { ArcScene } from './components/ArcScene'
import { Timeline } from './components/Timeline'
import { FilterPanel } from './components/FilterPanel'
import { PlaybackBar } from './components/PlaybackBar'
import { ShiftStrip } from './components/ShiftStrip'

const PLAYBACK_SPEED = 8 // simulation minutes per real second

function formatTime(minutes: number): string {
  const d = Math.floor(minutes / 1440)
  const h = Math.floor((minutes % 1440) / 60)
  const m = minutes % 60
  return d > 0 ? `Д${d} ${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}` : `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}`
}

function formatDuration(minutes: number): string {
  const h = Math.floor(minutes / 60)
  const m = minutes % 60
  return h > 0 ? `${h}ч ${m}м` : `${m}м`
}

function CrewDetailPanel({
  crew,
  segments,
  playbackMin,
  finding,
  resolved,
  onToggleResolution,
  onClose,
}: {
  crew: { crewId: string; crewName: string; depot: string; locomotiveId: string; locomotiveSeries: string } | null
  segments: CrewSegment[]
  playbackMin: number
  finding: SyntheticFinding | null
  resolved: boolean
  onToggleResolution: () => void
  onClose: () => void
}) {
  if (!crew) return null

  const crewSegments = segments.filter(s => s.crewId === crew.crewId)
  const totalWork = crewSegments.filter(s => s.kind === 'traveling').reduce((a, s) => a + (s.endMin - s.startMin), 0)
  const totalRest = crewSegments.filter(s => s.kind === 'rest_turnaround').reduce((a, s) => a + (s.endMin - s.startMin), 0)
  const violations = crewSegments.reduce((a, s) => a + s.violations, 0)
  const distance = crewSegments.reduce((a, s) => a + s.distanceKm, 0)

  return (
    <div style={styles.detailPanel}>
      <div style={styles.detailHeader}>
        <span style={styles.detailTitle}>{crew.crewName}</span>
        <button style={styles.closeBtn} onClick={onClose} aria-label="Закрыть">✕</button>
      </div>

      {finding && (
        <div style={{ ...styles.findingPanel, ...(finding.severity === 'critical' ? styles.findingCritical : {}) }}>
          <div style={styles.findingStatus}>
            <span>{finding.severity === 'critical' ? 'ТРЕБУЕТСЯ ВНИМАНИЕ' : 'ЗОНА ПРОВЕРКИ'}</span>
            <span style={styles.findingCode}>{finding.code}</span>
          </div>
          <div style={styles.findingTitle}>{finding.title}</div>
          <div style={styles.findingDetail}>{finding.detail}</div>
          <div style={styles.findingMeta}>Точка контроля: {finding.station} · {formatTime(finding.eventTime)}</div>
          <div style={styles.recommendation}>
            <span style={styles.recommendationLabel}>Рекомендация для демо-сценария</span>
            <span>{finding.recommendation}</span>
          </div>
          <div style={styles.whatIfRow}>
            <button style={{ ...styles.whatIfBtn, ...(resolved ? styles.whatIfBtnActive : {}) }} onClick={onToggleResolution}>
              {resolved ? '✓ Резервная бригада назначена' : 'Назначить резервную бригаду'}
            </button>
            <span style={styles.whatIfEffect}>{resolved ? 'Предотвращено: до 42 мин переработки' : 'Моделирование эффекта решения'}</span>
          </div>
        </div>
      )}

      <div style={styles.detailMeta}>
        <div style={styles.metaRow}>
          <span style={styles.metaLabel}>Бригада:</span>
          <span style={styles.metaValue}>{crew.crewId}</span>
        </div>
        <div style={styles.metaRow}>
          <span style={styles.metaLabel}>Депо:</span>
          <span style={styles.metaValue}>{crew.depot}</span>
        </div>
        <div style={styles.metaRow}>
          <span style={styles.metaLabel}>Локомотив:</span>
          <span style={styles.metaValue}>{crew.locomotiveId} · {crew.locomotiveSeries}</span>
        </div>
        <div style={styles.metaRow}>
          <span style={styles.metaLabel}>Участков поездки:</span>
          <span style={styles.metaValue}>{crewSegments.filter(s => s.kind === 'traveling').length}</span>
        </div>
      </div>

      <ShiftStrip segments={crewSegments} playbackMin={playbackMin} finding={finding} resolved={resolved} />

      <div style={styles.statsGrid}>
        <StatCard label="Рабочее время" value={formatDuration(totalWork)} color="#00e87a" />
        <StatCard label="Отдых (оборот)" value={formatDuration(totalRest)} color="#ffcc00" />
        <StatCard label="Нарушения" value={violations > 0 ? String(violations) : 'нет'} color={violations > 0 ? '#ff3355' : '#4466aa'} />
        <StatCard label="Пробег" value={`${distance.toFixed(1)} км`} color="#4488cc" />
      </div>

      <div style={styles.sectionTitle}>Маршруты смен</div>
      <div style={styles.segmentsList}>
        {crewSegments.map((seg, idx) => {
          const isCurrent = playbackMin >= seg.startMin && playbackMin <= seg.endMin
          const isPast = playbackMin > seg.endMin
          const kindLabels: Record<string, string> = {
            traveling: '🚂 Поездка',
            rest_turnaround: '💤 Отдых на обороте',
            preparation: '📋 Подготовка',
            home_rest: '🏠 Домашний отдых',
          }
          return (
            <div key={idx} style={{
              ...styles.segmentItem,
              borderLeftColor: seg.violations > 0 ? '#ff3355' :
                seg.kind === 'traveling' ? '#00e87a' :
                seg.kind === 'rest_turnaround' ? '#ffcc00' : '#4488cc',
              opacity: isCurrent ? 1 : isPast ? 0.6 : 0.8,
            }}>
              <div style={styles.segmentTop}>
                <span style={{ ...styles.segmentKind, color: isCurrent ? '#fff' : undefined }}>
                  {kindLabels[seg.kind] || seg.kind}
                  {isCurrent && <span style={styles.liveBadge}>LIVE</span>}
                </span>
                <span style={styles.segmentTime}>
                  {formatTime(seg.startMin)} — {formatTime(seg.endMin)}
                </span>
              </div>
              <div style={styles.segmentRoute}>
                {seg.from} → {seg.to}
              </div>
              <div style={styles.segmentDetails}>
                <span>{isPast ? '✓ завершён' : isCurrent ? '● выполняется' : '○ запланирован'}</span>
                {seg.kind === 'traveling' && (
                  <>
                    <span>📏 {seg.distanceKm.toFixed(1)} км</span>
                    {seg.violations > 0 && <span style={{ color: '#ff3355' }}>⚠ {seg.violations} наруш.</span>}
                  </>
                )}
                {seg.kind === 'rest_turnaround' && (
                  <span>⏱ {formatDuration(seg.endMin - seg.startMin)}</span>
                )}
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function StatCard({ label, value, color }: { label: string; value: string; color: string }) {
  return (
    <div style={styles.statCard}>
      <div style={{ ...styles.statValue, color }}>{value}</div>
      <div style={styles.statLabel}>{label}</div>
    </div>
  )
}

function Kpi({ label, value, color }: { label: string; value: string; color: string }) {
  return (
    <div style={styles.kpi}>
      <span style={{ ...styles.kpiValue, color }}>{value}</span>
      <span style={styles.kpiLabel}>{label}</span>
    </div>
  )
}

export default function App() {
  const [vizState, setVizState] = useState<VizState | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [filter, setFilter] = useState<FilterState>({ depot: null, kind: null, onlyViolations: false })
  const [selectedCrew, setSelectedCrew] = useState<string | null>(null)
  const [playbackMin, setPlaybackMin] = useState(0)
  const [playing, setPlaying] = useState(false)
  const [speed, setSpeed] = useState(8)
  const [autoRotate, setAutoRotate] = useState(false)
  const [showTimeline, setShowTimeline] = useState(true)
  const [presentationMode, setPresentationMode] = useState(false)
  const [resolvedFindingCrew, setResolvedFindingCrew] = useState<string | null>(null)
  const [storyActive, setStoryActive] = useState(false)
  const [storyStep, setStoryStep] = useState(0)
  const storyTimeoutsRef = useRef<number[]>([])
  const rafRef = useRef<number | null>(null)
  const lastTsRef = useRef<number | null>(null)

  useEffect(() => {
    Promise.all([
      fetch('./network.json').then(r => r.json() as Promise<NetworkData>),
      fetch('./sim.json').then(r => r.json() as Promise<Tu3Row[]>),
      fetch('./findings.json').then(r => r.json() as Promise<SyntheticFinding[]>),
    ])
      .then(([network, rows, findings]) => {
        const state = { ...buildVizState(rows, network), findings }
        const params = new URLSearchParams(window.location.search)
        const requestedCrew = params.get('crew')
        const investigation = params.get('mode') === 'investigation' || params.get('mode') === 'presentation'
        const requestedFinding = state.findings.find(finding => finding.crewId === requestedCrew)

        setVizState(state)
        if (requestedCrew && state.crews.includes(requestedCrew)) {
          const firstSegment = state.segments.find(segment => segment.crewId === requestedCrew)
          setSelectedCrew(requestedCrew)
          setPlaybackMin(requestedFinding?.eventTime ?? firstSegment?.startMin ?? state.minTime)
          const isPresentation = params.get('mode') === 'presentation'
          setPlaying(false)
          setShowTimeline(investigation && !isPresentation)
          setPresentationMode(isPresentation)
        } else {
          setPlaybackMin(state.minTime)
          setPlaying(true)
        }
      })
      .catch(err => setError(String(err)))
  }, [])

  const tick = useCallback((ts: number) => {
    if (lastTsRef.current === null) {
      lastTsRef.current = ts
    }
    const dt = (ts - lastTsRef.current) / 1000
    lastTsRef.current = ts

    setPlaybackMin(prev => {
      if (!vizState) return prev
      const next = prev + dt * speed
      if (next >= vizState.maxTime) {
        setPlaying(false)
        return vizState.maxTime
      }
      return next
    })
    rafRef.current = requestAnimationFrame(tick)
  }, [vizState, speed])

  useEffect(() => {
    if (playing) {
      lastTsRef.current = null
      rafRef.current = requestAnimationFrame(tick)
    } else if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current)
    }
    return () => { if (rafRef.current !== null) cancelAnimationFrame(rafRef.current) }
  }, [playing, tick])

  useEffect(() => () => {
    storyTimeoutsRef.current.forEach(window.clearTimeout)
  }, [])

  // This hook must execute on every render. Previously it was placed after the
  // loading return, violating React's hook order after simulation data loaded
  // and causing React to unmount the entire application as a blank screen.
  const selectedCrewData = useMemo(() => {
    if (!selectedCrew || !vizState) return null
    const seg = vizState.segments.find(s => s.crewId === selectedCrew)
    return seg ? {
      crewId: selectedCrew,
      crewName: seg.crewName,
      depot: seg.depot,
      locomotiveId: seg.locomotiveId,
      locomotiveSeries: seg.locomotiveSeries,
    } : null
  }, [selectedCrew, vizState])

  const crewOptions = useMemo(() => {
    if (!vizState) return []
    return vizState.crews.map(crewId => {
      const firstSegment = vizState.segments.find(segment => segment.crewId === crewId)
      const violations = vizState.segments
        .filter(segment => segment.crewId === crewId)
        .reduce((total, segment) => total + segment.violations, 0)
      return {
        crewId,
        name: firstSegment?.crewName ?? crewId,
        firstWorkMin: firstSegment?.startMin ?? vizState.minTime,
        violations,
      }
    }).sort((a, b) => b.violations - a.violations || a.name.localeCompare(b.name, 'ru'))
  }, [vizState])

  const investigateCrew = (crewId: string) => {
    if (!vizState) return
    storyTimeoutsRef.current.forEach(window.clearTimeout)
    storyTimeoutsRef.current = []
    setStoryActive(false)
    setStoryStep(0)
    if (!crewId) {
      setSelectedCrew(null)
      return
    }
    const option = crewOptions.find(item => item.crewId === crewId)
    setSelectedCrew(crewId)
    setPlaying(false)
    setPlaybackMin(option?.firstWorkMin ?? vizState.minTime)
  }

  const updateDemoUrl = (crewId: string | null, mode: 'investigation' | 'presentation' | null) => {
    const url = new URL(window.location.href)
    if (crewId) url.searchParams.set('crew', crewId)
    else url.searchParams.delete('crew')
    if (mode) url.searchParams.set('mode', mode)
    else url.searchParams.delete('mode')
    window.history.replaceState(null, '', url)
  }

  const showProblemTrip = () => {
    if (!vizState || vizState.findings.length === 0) return
    const finding = [...vizState.findings].sort((a, b) => {
      if (a.severity !== b.severity) return a.severity === 'critical' ? -1 : 1
      return a.eventTime - b.eventTime
    })[0]
    updateDemoUrl(finding.crewId, presentationMode ? 'presentation' : 'investigation')
    setSelectedCrew(finding.crewId)
    setResolvedFindingCrew(null)
    setStoryActive(false)
    setStoryStep(0)
    setPlaying(false)
    setPlaybackMin(finding.eventTime)
    setShowTimeline(true)
  }

  const toggleResolution = () => {
    if (!selectedCrew) return
    setResolvedFindingCrew(current => current === selectedCrew ? null : selectedCrew)
  }

  const togglePresentation = () => {
    if (!vizState) return
    const next = !presentationMode
    setPresentationMode(next)
    setShowTimeline(!next)
    if (next) {
      showProblemTrip()
      updateDemoUrl(selectedCrew ?? vizState.findings[0]?.crewId ?? null, 'presentation')
    } else {
      updateDemoUrl(selectedCrew, selectedCrew ? 'investigation' : null)
    }
  }

  const playStory = () => {
    if (!vizState || vizState.findings.length === 0) return
    storyTimeoutsRef.current.forEach(window.clearTimeout)
    storyTimeoutsRef.current = []

    const finding = [...vizState.findings].sort((a, b) => {
      if (a.severity !== b.severity) return a.severity === 'critical' ? -1 : 1
      return a.eventTime - b.eventTime
    })[0]
    const crewSegments = vizState.segments
      .filter(segment => segment.crewId === finding.crewId && segment.kind !== 'home_rest')
      .sort((a, b) => a.startMin - b.startMin)
    const keyMoments = [crewSegments[0]?.startMin, crewSegments.find(segment => segment.kind === 'traveling')?.startMin, finding.eventTime]
      .filter((time): time is number => time !== undefined)

    setSelectedCrew(finding.crewId)
    setResolvedFindingCrew(null)
    setShowTimeline(false)
    setPresentationMode(true)
    updateDemoUrl(finding.crewId, 'presentation')
    setPlaying(false)
    setStoryStep(0)
    setStoryActive(true)

    keyMoments.forEach((time, index) => {
      storyTimeoutsRef.current.push(window.setTimeout(() => {
        setPlaybackMin(time)
        setStoryStep(index)
      }, index * 2600))
    })
    storyTimeoutsRef.current.push(window.setTimeout(() => {
      setPlaybackMin(finding.eventTime)
      setStoryStep(2)
      setStoryActive(false)
    }, keyMoments.length * 2600))
  }

  const selectedFinding = useMemo(
    () => vizState?.findings.find(finding => finding.crewId === selectedCrew) ?? null,
    [vizState, selectedCrew],
  )

  const networkKpis = useMemo(() => {
    if (!vizState) return { crews: 0, trips: 0, findings: 0, preventedMinutes: 0 }
    const trips = vizState.segments.filter(segment => segment.kind === 'traveling').length
    const preventedMinutes = resolvedFindingCrew ? 42 : 96
    return { crews: vizState.crews.length, trips, findings: vizState.findings.length, preventedMinutes }
  }, [vizState, resolvedFindingCrew])

  const storyCopy = [
    { index: '01', title: 'Бригада приняла локомотив', text: 'Подготовка завершена, маршрут готов к отправлению.' },
    { index: '02', title: 'Поезд следует по плечу', text: 'Система сопоставляет график, маршрут и режим труда.' },
    { index: '03', title: 'Обнаружен риск режима труда и отдыха', text: 'Контрольная точка требует диспетчерской проверки.' },
  ][storyStep]

  const resetDemo = () => {
    storyTimeoutsRef.current.forEach(window.clearTimeout)
    storyTimeoutsRef.current = []
    setSelectedCrew(null)
    setResolvedFindingCrew(null)
    setStoryActive(false)
    setStoryStep(0)
    setPlaying(false)
    setPlaybackMin(vizState?.minTime ?? 0)
    setFilter({ depot: null, kind: null, onlyViolations: false })
  }

  if (error) return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', color: '#ff6677' }}>
      Ошибка загрузки данных: {error}
    </div>
  )

  if (!vizState) return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', color: '#4466aa', flexDirection: 'column', gap: 12 }}>
      <div style={{ width: 40, height: 40, border: '3px solid #4466aa', borderTopColor: '#00e87a', borderRadius: '50%', animation: 'spin 0.8s linear infinite' }} />
      Загрузка симуляции…
      <style>{`@keyframes spin { to { transform: rotate(360deg) } }`}</style>
    </div>
  )

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden' }}>
      {/* Top bar */}
      <div style={styles.topBar}>
        <span style={styles.topTitle}>Цифровой двойник · Оборот локомотивных бригад</span>
        <span style={styles.syntheticBadge}>СИНТЕТИЧЕСКИЕ ДАННЫЕ</span>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <button style={{ ...styles.topBtn, ...(presentationMode ? styles.topBtnActive : {}) }} onClick={togglePresentation}>
            {presentationMode ? '◱ Рабочий режим' : '▣ Режим презентации'}
          </button>
          {!presentationMode && (
            <button
              style={{ ...styles.topBtn, ...(showTimeline ? styles.topBtnActive : {}) }}
              onClick={() => setShowTimeline(v => !v)}
            >📊 Таймлайн</button>
          )}
          <button style={{ ...styles.topBtn, ...(storyActive ? styles.topBtnActive : {}) }} onClick={playStory}>
            {storyActive ? '◉ История рейса' : '▶ История рейса'}
          </button>
          <button
            style={{ ...styles.topBtn, ...(autoRotate ? styles.topBtnActive : {}) }}
            onClick={() => setAutoRotate(value => !value)}
            title="Вращение выключено по умолчанию, чтобы карта не мешала анализу"
          >{autoRotate ? '⏸ Остановить обзор' : '◌ Обзор карты'}</button>
          <button style={styles.topBtn} onClick={resetDemo} title="Вернуть стартовое состояние демонстрации">
            ↺ Сбросить демо
          </button>
        </div>
      </div>

      <div style={styles.kpiBar}>
        <Kpi label="Бригад под контролем" value={String(networkKpis.crews)} color="#88b8ff" />
        <Kpi label="Поездок в сценарии" value={String(networkKpis.trips)} color="#00e87a" />
        <Kpi label="Рейсов требуют проверки" value={String(networkKpis.findings)} color="#ffb84d" />
        <Kpi label="Риск предотвращаемо" value={`до ${networkKpis.preventedMinutes} мин`} color={resolvedFindingCrew ? '#00e87a' : '#ffcc66'} />
        <span style={styles.kpiNote}>Расчёт для синтетического сценария</span>
      </div>

      {!presentationMode && (
      <div style={styles.workflowBar}>
        <div>
          <div style={styles.workflowTitle}>Проверка работы бригады</div>
          <div style={styles.workflowHint}>1. Выберите бригаду · 2. Откройте её маршрут · 3. Запустите или перемотайте смену</div>
        </div>
        <button style={styles.problemBtn} onClick={showProblemTrip}>
          ⚠ Показать проблемный рейс
        </button>
        <label style={styles.crewPickerLabel}>
          <span>Бригада</span>
          <select
            value={selectedCrew ?? ''}
            onChange={event => investigateCrew(event.target.value)}
            style={styles.crewPicker}
          >
            <option value="">Выберите для анализа…</option>
            {crewOptions.map(option => (
              <option key={option.crewId} value={option.crewId}>
                {option.name} · {option.crewId}{option.violations > 0 ? ` · ⚠ ${option.violations}` : ''}
              </option>
            ))}
          </select>
        </label>
        {selectedCrewData && (
          <button style={styles.analyzeBtn} onClick={() => setShowTimeline(true)}>
            Открыть карточку маршрута →
          </button>
        )}
      </div>
      )}

      {/* Main area */}
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden', minHeight: 0 }}>
        {!presentationMode && <FilterPanel vizState={vizState} filter={filter} onChange={setFilter} />}

        {/* Central area: 3D scene + Timeline */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', position: 'relative', minWidth: 0 }}>
          {/* 3D Scene - explicit height to avoid Canvas 0-height bug */}
          <div style={{
            height: showTimeline ? '55%' : '100%',
            minHeight: 200,
            position: 'relative',
            flexShrink: 0,
          }}>
            <ArcScene
              vizState={vizState}
              filter={filter}
              selectedCrew={selectedCrew}
              playbackMin={playbackMin}
              autoRotate={autoRotate}
              controlStation={selectedFinding?.station ?? null}
              resolved={resolvedFindingCrew === selectedCrew}
              onSelectCrew={setSelectedCrew}
            />
            {/* Overlay hint */}
            {presentationMode && selectedCrewData && (
              <div style={styles.presentationTitle}>
                <div style={styles.presentationKicker}>ЦИФРОВОЙ ДВОЙНИК РЕЖИМА РАБОТЫ</div>
                <div style={styles.presentationName}>{selectedCrewData.crewName} · {selectedCrewData.locomotiveId}</div>
                <div style={styles.presentationRoute}>Контроль маршрута и режима труда на синтетической железной дороге</div>
              </div>
            )}
            {storyActive && (
              <div style={styles.storyOverlay}>
                <div style={styles.storyStep}>ЭТАП {storyCopy.index} / 03</div>
                <div style={styles.storyTitle}>{storyCopy.title}</div>
                <div style={styles.storyText}>{storyCopy.text}</div>
              </div>
            )}
            {resolvedFindingCrew === selectedCrew && selectedFinding && (
              <div style={styles.resolutionOverlay}>
                <div style={styles.resolutionStatus}>РЕШЕНИЕ СМОДЕЛИРОВАНО</div>
                <div style={styles.resolutionTitle}>Резервная бригада назначена на следующее плечо</div>
                <div style={styles.resolutionText}>Контрольная точка: {selectedFinding.station} · предотвращено до 42 мин переработки</div>
              </div>
            )}
            <div style={styles.hint}>
              Перетащите для поворота · Колесо — масштаб · Клик на арку — выбор бригады
            </div>
          </div>

          {/* Timeline panel */}
          {showTimeline && (
            <div style={{ flex: 1, overflow: 'hidden', borderTop: '1px solid #1a2a4a', display: 'flex', flexDirection: 'column' }}>
              <div style={styles.panelHeader}>
                {selectedCrew ? (
                  <button style={styles.backBtn} onClick={() => setSelectedCrew(null)}>
                    ← Назад к списку
                  </button>
                ) : (
                  'График работы бригад'
                )}
              </div>
              {selectedCrew ? (
                <div style={{ flex: 1, overflow: 'auto' }}>
                  <CrewDetailPanel
                    crew={selectedCrewData}
                    segments={vizState.segments}
                    playbackMin={playbackMin}
                    finding={selectedFinding}
                    resolved={resolvedFindingCrew === selectedCrew}
                    onToggleResolution={toggleResolution}
                    onClose={() => setSelectedCrew(null)}
                  />
                </div>
              ) : (
                <Timeline
                  vizState={vizState}
                  filter={filter}
                  selectedCrew={selectedCrew}
                  playbackMin={playbackMin}
                  onSelectCrew={setSelectedCrew}
                />
              )}
            </div>
          )}

          {/* Crew Detail Panel (right side when timeline hidden) */}
          {!presentationMode && !showTimeline && selectedCrew && (
            <div style={styles.detailPanelWrapper}>
              <CrewDetailPanel
                crew={selectedCrewData}
                segments={vizState.segments}
                playbackMin={playbackMin}
                finding={selectedFinding}
                resolved={resolvedFindingCrew === selectedCrew}
                onToggleResolution={toggleResolution}
                onClose={() => setSelectedCrew(null)}
              />
            </div>
          )}
        </div>
      </div>

      {!presentationMode && (
        <PlaybackBar
          vizState={vizState}
          playbackMin={playbackMin}
          playing={playing}
          onChangeMin={setPlaybackMin}
          onTogglePlay={() => setPlaying(v => !v)}
          speed={speed}
          onSpeedChange={setSpeed}
          onReset={() => { setPlaybackMin(vizState?.minTime ?? 0); setPlaying(false); }}
        />
      )}
    </div>
  )
}

const styles: Record<string, React.CSSProperties> = {
  topBar: {
    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
    padding: '0 16px', height: 40, background: '#0d1220',
    borderBottom: '1px solid #1a2a4a', flexShrink: 0,
  },
  topTitle: { fontSize: 12, color: '#8899bb', letterSpacing: 0.5 },
  syntheticBadge: {
    fontSize: 9, color: '#ffcc00', background: 'rgba(255,204,0,0.12)',
    border: '1px solid rgba(255,204,0,0.3)', borderRadius: 3,
    padding: '2px 7px', letterSpacing: 1.5, fontWeight: 700,
  },
  topBtn: {
    background: 'transparent', border: '1px solid #1a3060',
    color: '#6688aa', padding: '3px 10px', borderRadius: 4,
    cursor: 'pointer', fontSize: 11,
  },
  topBtnActive: { background: '#1a3060', color: '#c8d8ff', borderColor: '#4488ff' },
  kpiBar: {
    height: 48, padding: '0 16px', display: 'flex', alignItems: 'center', gap: 26,
    background: '#0a101d', borderBottom: '1px solid #172b48', flexShrink: 0,
  },
  kpi: { display: 'flex', alignItems: 'baseline', gap: 7, whiteSpace: 'nowrap' },
  kpiValue: { fontSize: 16, fontWeight: 700, fontVariantNumeric: 'tabular-nums' },
  kpiLabel: { color: '#7386a8', fontSize: 10 },
  kpiNote: { marginLeft: 'auto', color: '#526583', fontSize: 9, letterSpacing: 0.4 },
  workflowBar: {
    minHeight: 62, padding: '8px 16px', background: '#101828', borderBottom: '1px solid #1a2a4a',
    display: 'flex', alignItems: 'center', gap: 16, flexShrink: 0,
  },
  workflowTitle: { color: '#c8d8ff', fontSize: 13, fontWeight: 700 },
  workflowHint: { color: '#6688aa', fontSize: 10, marginTop: 3 },
  crewPickerLabel: {
    marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 8,
    color: '#6688aa', fontSize: 10, whiteSpace: 'nowrap',
  },
  crewPicker: {
    width: 270, background: '#0a1828', border: '1px solid #2b4f87', borderRadius: 5,
    color: '#e0e8ff', padding: '6px 8px', fontSize: 11, cursor: 'pointer',
  },
  problemBtn: {
    background: '#3a2410', border: '1px solid #d88732', borderRadius: 5,
    color: '#ffd18a', padding: '6px 10px', fontSize: 11, cursor: 'pointer', whiteSpace: 'nowrap',
  },
  analyzeBtn: {
    background: '#1a3060', border: '1px solid #4488ff', borderRadius: 5,
    color: '#e0e8ff', padding: '6px 10px', fontSize: 11, cursor: 'pointer', whiteSpace: 'nowrap',
  },
  findingPanel: {
    margin: '12px 16px 0', padding: 12, borderRadius: 6,
    border: '1px solid #d88732', background: '#261b12', color: '#f4d2a0',
  },
  findingCritical: {
    borderColor: '#ff3355', background: '#2a1018', color: '#ffb8c2',
  },
  findingStatus: {
    display: 'flex', justifyContent: 'space-between', alignItems: 'center',
    fontSize: 9, letterSpacing: 1, fontWeight: 700, color: '#ffcc00',
  },
  findingCode: { color: '#aa88aa', letterSpacing: 0.5 },
  findingTitle: { marginTop: 6, fontSize: 12, fontWeight: 700, color: '#fff1db' },
  findingDetail: { marginTop: 6, fontSize: 10, lineHeight: 1.45 },
  findingMeta: { marginTop: 8, fontSize: 10, color: '#c59b70' },
  recommendation: {
    display: 'flex', flexDirection: 'column', gap: 4, marginTop: 10, paddingTop: 8,
    borderTop: '1px solid rgba(255,204,0,0.2)', fontSize: 10, lineHeight: 1.4,
  },
  recommendationLabel: { color: '#ffcc00', fontSize: 9, textTransform: 'uppercase', letterSpacing: 0.5 },
  whatIfRow: { display: 'flex', flexDirection: 'column', gap: 6, marginTop: 12 },
  whatIfBtn: {
    background: '#3a2410', border: '1px solid #d88732', borderRadius: 4,
    color: '#ffd18a', padding: '6px 8px', fontSize: 10, cursor: 'pointer', textAlign: 'left',
  },
  whatIfBtnActive: { background: '#103421', borderColor: '#00b85f', color: '#8df0ba' },
  whatIfEffect: { color: '#a4b8cc', fontSize: 9 },

  presentationTitle: {
    position: 'absolute', top: 18, left: 18, padding: '12px 14px', maxWidth: 410,
    background: 'rgba(5, 13, 25, 0.82)', borderLeft: '3px solid #4d9cff', pointerEvents: 'none',
  },
  presentationKicker: { color: '#6faaf0', fontSize: 9, fontWeight: 700, letterSpacing: 1.15 },
  presentationName: { color: '#f2f7ff', fontSize: 17, fontWeight: 700, marginTop: 5 },
  presentationRoute: { color: '#a5b9d5', fontSize: 10, marginTop: 5, lineHeight: 1.4 },
  storyOverlay: {
    position: 'absolute', top: 122, left: 18, width: 290, padding: '12px 14px',
    background: 'rgba(8, 16, 30, 0.94)', border: '1px solid #2b5a8f', borderLeft: '3px solid #62a8ff',
    boxShadow: '0 12px 30px rgba(0,0,0,0.28)', pointerEvents: 'none',
  },
  storyStep: { color: '#78adf2', fontSize: 9, fontWeight: 700, letterSpacing: 1.1 },
  storyTitle: { color: '#edf4ff', marginTop: 6, fontSize: 13, fontWeight: 700, lineHeight: 1.25 },
  storyText: { color: '#a8bad2', marginTop: 6, fontSize: 10, lineHeight: 1.45 },
  resolutionOverlay: {
    position: 'absolute', top: 14, right: 16, width: 310, padding: '12px 14px',
    background: 'rgba(9, 35, 25, 0.94)', border: '1px solid #1b8451', borderLeft: '3px solid #00db73',
    boxShadow: '0 12px 30px rgba(0,0,0,0.28)', pointerEvents: 'none',
  },
  resolutionStatus: { color: '#7bf0b6', fontSize: 9, fontWeight: 700, letterSpacing: 1.1 },
  resolutionTitle: { color: '#ebfff5', marginTop: 6, fontSize: 12, fontWeight: 700, lineHeight: 1.3 },
  resolutionText: { color: '#a7d9bd', marginTop: 6, fontSize: 10, lineHeight: 1.45 },
  hint: {
    position: 'absolute', bottom: 10, left: '50%', transform: 'translateX(-50%)',
    fontSize: 10, color: '#334466', pointerEvents: 'none',
    background: 'rgba(10,14,26,0.7)', padding: '3px 10px', borderRadius: 10,
  },
  panelHeader: {
    padding: '8px 12px', fontSize: 11, color: '#4466aa',
    borderBottom: '1px solid #1a2a4a', flexShrink: 0,
  },
  backBtn: {
    background: 'transparent', border: 'none', color: '#6688aa',
    cursor: 'pointer', fontSize: 11, padding: 0,
  },
  detailPanelWrapper: {
    width: 320,
    flexShrink: 0,
    borderLeft: '1px solid #1a2a4a',
    background: '#0a0e1a',
    overflowY: 'auto',
    height: '100%',
  },
  detailPanel: {
    background: '#0a0e1a',
    borderLeft: '1px solid #1a2a4a',
    width: 320,
    flexShrink: 0,
    overflowY: 'auto',
    display: 'flex',
    flexDirection: 'column',
  },
  detailHeader: {
    display: 'flex', justifyContent: 'space-between', alignItems: 'center',
    padding: '12px 16px', borderBottom: '1px solid #1a2a4a',
  },
  detailTitle: { fontSize: 14, fontWeight: 600, color: '#c8d8ff' },
  closeBtn: {
    background: 'transparent', border: '1px solid #1a3060', color: '#8899bb',
    width: 24, height: 24, borderRadius: 4, cursor: 'pointer', fontSize: 11,
    display: 'flex', alignItems: 'center', justifyContent: 'center',
  },
  detailMeta: {
    padding: '12px 16px', borderBottom: '1px solid #1a2a4a',
    display: 'flex', flexDirection: 'column', gap: 6,
  },
  metaRow: {
    display: 'flex', justifyContent: 'space-between', fontSize: 11,
  },
  metaLabel: { color: '#4466aa' },
  metaValue: { color: '#c8d8ff', fontWeight: 500 },
  statsGrid: {
    display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8,
    padding: '12px 16px', borderBottom: '1px solid #1a2a4a',
  },
  statCard: {
    background: '#0d1220', border: '1px solid #1a3060', borderRadius: 6,
    padding: '10px 8px', textAlign: 'center',
  },
  statValue: { fontSize: 16, fontWeight: 700 },
  statLabel: { fontSize: 9, color: '#6688aa', marginTop: 2 },
  sectionTitle: {
    padding: '10px 16px', fontSize: 10, color: '#4466aa',
    textTransform: 'uppercase', letterSpacing: 1,
  },
  segmentsList: {
    padding: '8px 12px', display: 'flex', flexDirection: 'column', gap: 8,
  },
  segmentItem: {
    background: '#0d1220', border: '1px solid #1a3060', borderRadius: 6,
    padding: '10px 12px', borderLeftWidth: 4,
    transition: 'opacity 0.2s, transform 0.1s',
  },
  segmentTop: {
    display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4,
  },
  segmentKind: { fontSize: 11, fontWeight: 600, color: '#c8d8ff' },
  liveBadge: {
    marginLeft: 6, padding: '1px 5px', fontSize: 8, color: '#fff',
    background: '#ff3355', borderRadius: 3, fontWeight: 700,
    animation: 'pulse 1s infinite',
  },
  segmentTime: { fontSize: 10, color: '#6688aa', fontVariant: 'tabular-nums' },
  segmentRoute: { fontSize: 12, color: '#88aacc', marginBottom: 4, fontWeight: 500 },
  segmentDetails: { fontSize: 10, color: '#6688aa', display: 'flex', gap: 12 },
}
