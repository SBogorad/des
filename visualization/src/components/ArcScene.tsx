import { useRef, useMemo, useState, useEffect, useCallback } from 'react'
import { Canvas, useFrame } from '@react-three/fiber'
import { OrbitControls, Line, Text } from '@react-three/drei'
import * as THREE from 'three'
import type { VizState, CrewSegment, FilterState } from '../types'
import { stationPos, SEGMENT_COLORS } from '../data/transform'

// ─── colours ────────────────────────────────────────────────────────────────

const STATION_COLORS: Record<string, string> = {
  MainDepot:  '#4499ff',
  Turnaround: '#ffcc00',
  Junction:   '#aa66ff',
  Waypoint:   '#1e3a5a',
  Terminal:   '#ff6644',
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/** Build a smooth 3-D arc that follows the actual rail waypoints.
 *  Each inter-waypoint span gets a small quadratic lift so the path
 *  visually "floats" above the track layer. */
function buildArcCurve(
  waypoints: [number, number, number][],
  lift: number,
): THREE.CatmullRomCurve3 {
  if (waypoints.length < 2) {
    return new THREE.CatmullRomCurve3([
      new THREE.Vector3(...waypoints[0]),
      new THREE.Vector3(...waypoints[0]),
    ])
  }
  // Insert a raised midpoint between every consecutive pair to create smooth arcs
  const pts: THREE.Vector3[] = []
  for (let i = 0; i < waypoints.length - 1; i++) {
    const a = waypoints[i]
    const b = waypoints[i + 1]
    pts.push(new THREE.Vector3(...a))
    // midpoint raised by lift
    pts.push(new THREE.Vector3((a[0] + b[0]) / 2, lift, (a[2] + b[2]) / 2))
  }
  pts.push(new THREE.Vector3(...waypoints[waypoints.length - 1]))
  return new THREE.CatmullRomCurve3(pts, false, 'catmullrom', 0.5)
}

// ─── station node ─────────────────────────────────────────────────────────

function StationNode({
  name, kind, pos, isControlPoint, resolved,
}: {
  name: string
  kind: string
  pos: [number, number, number]
  isControlPoint: boolean
  resolved: boolean
}) {
  const markerRef = useRef<THREE.Mesh>(null)
  const color = isControlPoint ? (resolved ? '#00e87a' : '#ff3355') : (STATION_COLORS[kind] ?? '#1e3a5a')
  const isKey = kind === 'MainDepot' || kind === 'Turnaround' || isControlPoint
  const size = isControlPoint ? 0.25 : kind === 'MainDepot' ? 0.18 : kind === 'Turnaround' ? 0.15 : 0.07
  const glow = isControlPoint ? 3.2 : kind === 'MainDepot' ? 1.8 : kind === 'Turnaround' ? 1.4 : 0.6

  useFrame(({ clock }) => {
    if (!markerRef.current || !isControlPoint) return
    const pulse = 1 + Math.sin(clock.getElapsedTime() * 4) * 0.24
    markerRef.current.scale.setScalar(pulse)
  })

  return (
    <group position={pos}>
      <mesh ref={markerRef}>
        <sphereGeometry args={[size, 16, 16]} />
        <meshStandardMaterial color={color} emissive={color} emissiveIntensity={glow} />
      </mesh>
      {isControlPoint && (
        <mesh position={[0, 0.02, 0]} rotation={[-Math.PI / 2, 0, 0]}>
          <ringGeometry args={[0.32, 0.38, 32]} />
          <meshBasicMaterial color={color} transparent opacity={0.8} />
        </mesh>
      )}
      {isKey && (
        <Text position={[0, isControlPoint ? 0.58 : 0.38, 0]} fontSize={isControlPoint ? 0.2 : 0.18}
          color={isControlPoint ? color : '#e0f0ff'} anchorX="center" anchorY="bottom"
          outlineWidth={0.04} outlineColor="#000c1a">
          {isControlPoint ? `${name} · ${resolved ? 'РЕШЕНИЕ' : 'КОНТРОЛЬ'}` : name}
        </Text>
      )}
    </group>
  )
}

// ─── track layer ──────────────────────────────────────────────────────────

function TrackNetwork({ network }: { network: VizState['network'] }) {
  const lines = useMemo(() => Object.values(network.segments).map(seg => {
    const f = stationPos(network.stations[seg.from]?.name ?? '', network)
    const t = stationPos(network.stations[seg.to]?.name  ?? '', network)
    return { id: seg.id, f, t }
  }), [network])

  return (
    <>
      {lines.map(l => (
        <Line key={l.id} points={[l.f, l.t]} color="#1a3a5a" lineWidth={1.5} />
      ))}
    </>
  )
}

// ─── crew arc ─────────────────────────────────────────────────────────────

function CrewArc({
  seg, playbackMin, isSelected, isFaded, resolved, onClick,
}: {
  seg: CrewSegment
  playbackMin: number
  isSelected: boolean
  isFaded: boolean
  resolved: boolean
  onClick: () => void
}) {
  const travelerRef = useRef<THREE.Mesh>(null)
  const trailRef = useRef<THREE.Mesh>(null)
  const locomotiveRef = useRef<THREE.Group>(null)

  const isTraveling = seg.kind === 'traveling'
  const isSameStation = seg.from === seg.to

  const isProblemLeg = isSelected && seg.violations > 0
  const baseColor = isProblemLeg && resolved ? '#00e87a' : seg.violations > 0 ? '#ff3355' : SEGMENT_COLORS[seg.kind]
  const lineColor = isProblemLeg ? baseColor : isSelected ? '#ffffff' : baseColor

  // Lift arcs higher than track lines so they're clearly readable
  const lift = isSelected ? 1.2 : 0.7

  const curve = useMemo(() => {
    if (isSameStation || !seg.waypoints || seg.waypoints.length < 2) return null
    return buildArcCurve(seg.waypoints, lift)
  }, [seg.waypoints, lift, isSameStation])

  const points = useMemo(() => {
    if (!curve) return []
    return curve.getPoints(80)
  }, [curve])

  const isActive = playbackMin >= seg.startMin && playbackMin <= seg.endMin

  useFrame(() => {
    if (!travelerRef.current) return
    if (!curve || !isTraveling) {
      travelerRef.current.visible = false
      if (trailRef.current) trailRef.current.visible = false
      if (locomotiveRef.current) locomotiveRef.current.visible = false
      return
    }
    const duration = seg.endMin - seg.startMin
    if (duration <= 0) { travelerRef.current.visible = false; return }

    const t = Math.max(0, Math.min(1, (playbackMin - seg.startMin) / duration))
    travelerRef.current.visible = isActive && !isSelected
    if (trailRef.current) trailRef.current.visible = isActive
    if (locomotiveRef.current) locomotiveRef.current.visible = isActive && isSelected

    if (isActive) {
      const pos = curve.getPoint(t)
      travelerRef.current.position.copy(pos)
      if (trailRef.current) {
        const trailT = Math.max(0, t - 0.04)
        trailRef.current.position.copy(curve.getPoint(trailT))
      }
      if (locomotiveRef.current) {
        locomotiveRef.current.position.copy(pos)
        const forward = curve.getTangent(Math.min(1, t + 0.01))
        locomotiveRef.current.rotation.y = Math.atan2(forward.x, forward.z)
      }
    }
  })

  if (isSameStation || points.length === 0) return null

  const opacity = isFaded ? 0.18 : isSelected ? 1.0 : 0.75
  const lineW   = isSelected ? 5 : seg.violations > 0 ? 3.5 : 2.5

  return (
    <group onClick={onClick}>
      <Line
        points={points}
        color={lineColor}
        lineWidth={lineW}
        transparent
        opacity={opacity}
      />
      {/* traveler dot */}
      <mesh ref={travelerRef} visible={false}>
        <sphereGeometry args={[isSelected ? 0.22 : 0.14, 14, 14]} />
        <meshStandardMaterial
          color={lineColor}
          emissive={lineColor}
          emissiveIntensity={isSelected ? 4 : 2.5}
          transparent
          opacity={opacity}
        />
      </mesh>
      {/* Selected crew uses a small locomotive-shaped marker instead of a generic dot. */}
      <group ref={locomotiveRef} visible={false}>
        <mesh position={[0, 0.05, 0]}>
          <boxGeometry args={[0.32, 0.16, 0.52]} />
          <meshStandardMaterial color={lineColor} emissive={lineColor} emissiveIntensity={2.8} />
        </mesh>
        <mesh position={[0, 0.17, -0.05]}>
          <boxGeometry args={[0.24, 0.12, 0.22]} />
          <meshStandardMaterial color="#e9f5ff" emissive="#b8e2ff" emissiveIntensity={1.6} />
        </mesh>
        <Text position={[0, 0.48, 0]} fontSize={0.15} color="#f2f8ff" anchorX="center" anchorY="bottom"
          outlineWidth={0.035} outlineColor="#06101e">
          {seg.locomotiveId} · {seg.crewName.split(' ')[0]}
        </Text>
      </group>
      {/* glow trail (slightly behind the traveler) */}
      <mesh ref={trailRef} visible={false}>
        <sphereGeometry args={[isSelected ? 0.30 : 0.20, 10, 10]} />
        <meshStandardMaterial
          color={lineColor}
          emissive={lineColor}
          emissiveIntensity={1.5}
          transparent
          opacity={opacity * 0.35}
        />
      </mesh>
    </group>
  )
}

// ─── full 3-D scene ───────────────────────────────────────────────────────

function Scene({
  vizState, filter, selectedCrew, playbackMin, autoRotate, controlStation, resolved, onSelectCrew,
}: {
  vizState: VizState
  filter: FilterState
  selectedCrew: string | null
  playbackMin: number
  autoRotate: boolean
  controlStation: string | null
  resolved: boolean
  onSelectCrew: (id: string | null) => void
}) {
  const network  = vizState.network
  const stations = Object.values(network.stations)

  const segments = useMemo(() => vizState.segments.filter(seg => {
    if (filter.depot && seg.depot !== filter.depot) return false
    if (filter.kind  && seg.kind  !== filter.kind)  return false
    if (filter.onlyViolations && seg.violations === 0) return false
    if (seg.kind === 'home_rest' || seg.kind === 'preparation') return false
    return true
  }), [vizState.segments, filter])

  const hasFocus = selectedCrew !== null

  return (
    <>
      <color attach="background" args={['#080e1c']} />

      {/* Ambient + directional lights */}
      <ambientLight intensity={0.6} color="#8ab4cc" />
      <directionalLight position={[8, 18, 8]}  intensity={1.8} color="#ffffff" />
      <directionalLight position={[-8, 12, -4]} intensity={0.9} color="#a8ccff" />
      <pointLight position={[0, 14, 0]} intensity={2} color="#88bbff" distance={55} />

      {/* Ground plane — subtle grid feel */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, -0.05, 0]}>
        <planeGeometry args={[30, 10]} />
        <meshStandardMaterial color="#060d1a" transparent opacity={0.85} />
      </mesh>

      <TrackNetwork network={network} />

      {stations.map(st => (
        <StationNode
          key={st.id}
          name={st.name}
          kind={st.kind}
          pos={stationPos(st.name, network)}
          isControlPoint={st.name === controlStation}
          resolved={resolved}
        />
      ))}

      {segments.map((seg, i) => (
        <CrewArc
          key={`${seg.crewId}-${seg.startMin}-${i}`}
          seg={seg}
          playbackMin={playbackMin}
          isSelected={selectedCrew === seg.crewId}
          isFaded={hasFocus && selectedCrew !== seg.crewId}
          resolved={resolved}
          onClick={() => onSelectCrew(selectedCrew === seg.crewId ? null : seg.crewId)}
        />
      ))}

      <OrbitControls
        autoRotate={autoRotate}
        autoRotateSpeed={0.3}
        enablePan
        enableZoom
        minDistance={3}
        maxDistance={35}
        maxPolarAngle={Math.PI / 2.1}
      />
    </>
  )
}

// ─── public component ─────────────────────────────────────────────────────

export function ArcScene({
  vizState, filter, selectedCrew, playbackMin, autoRotate, controlStation, resolved, onSelectCrew,
}: {
  vizState: VizState
  filter: FilterState
  selectedCrew: string | null
  playbackMin: number
  autoRotate: boolean
  controlStation: string | null
  resolved: boolean
  onSelectCrew: (id: string | null) => void
}) {
  const [webglOk, setWebglOk] = useState<boolean | null>(null)

  useEffect(() => {
    try {
      const c = document.createElement('canvas')
      const gl = c.getContext('webgl2') || c.getContext('webgl')
      setWebglOk(!!gl)
    } catch { setWebglOk(false) }
  }, [])

  if (webglOk === null) return (
    <div style={{ background: '#080e1c', width: '100%', height: '100%' }} />
  )
  if (!webglOk) return (
    <FallbackMap
      vizState={vizState}
      filter={filter}
      selectedCrew={selectedCrew}
      onSelectCrew={onSelectCrew}
    />
  )

  return (
    <Canvas
      // Camera positioned to see the full east–west diagonal of the network.
      // Network spans x[-12,12] z[-4.5,4.5]. We sit above and slightly south.
      camera={{ position: [0, 10, 16], fov: 52 }}
      style={{ width: '100%', height: '100%', background: '#080e1c' }}
      onCreated={({ gl }) => {
        gl.setClearColor('#080e1c')
        gl.toneMapping = THREE.ACESFilmicToneMapping
        gl.toneMappingExposure = 1.1
      }}
    >
      <Scene
        vizState={vizState}
        filter={filter}
        selectedCrew={selectedCrew}
        playbackMin={playbackMin}
        autoRotate={autoRotate}
        controlStation={controlStation}
        resolved={resolved}
        onSelectCrew={onSelectCrew}
      />
    </Canvas>
  )
}

// ─── 2-D SVG fallback ─────────────────────────────────────────────────────

function FallbackMap({
  vizState, filter, selectedCrew, onSelectCrew,
}: {
  vizState: VizState
  filter: FilterState
  selectedCrew: string | null
  onSelectCrew: (id: string | null) => void
}) {
  const network  = vizState.network
  const stations = Object.values(network.stations)
  const W = 820, H = 320, PAD = 44

  const project = useCallback((name: string): [number, number] => {
    const st = stations.find(s => s.name === name)
    if (!st) return [W / 2, H / 2]
    const [cx, cy] = st.coords
    const x = PAD + ((cx - 50)  / (1700 - 50))  * (W - 2 * PAD)
    const y = PAD + ((cy - 60)  / (220  - 60))  * (H - 2 * PAD)
    return [x, y]
  }, [stations, W, H, PAD])

  const segs = vizState.segments.filter(s => {
    if (filter.depot && s.depot !== filter.depot) return false
    if (filter.kind  && s.kind  !== filter.kind)  return false
    if (filter.onlyViolations && s.violations === 0) return false
    if (s.kind === 'home_rest' || s.kind === 'preparation') return false
    return true
  })

  return (
    <div style={{
      width: '100%', height: '100%', overflow: 'auto',
      background: '#080e1c', display: 'flex', alignItems: 'center', justifyContent: 'center',
    }}>
      <svg width={W} height={H} style={{ maxWidth: '100%' }}>
        {/* tracks */}
        {Object.values(network.segments).map(s => {
          const f = project(network.stations[s.from]?.name ?? '')
          const t = project(network.stations[s.to]?.name  ?? '')
          return <line key={s.id} x1={f[0]} y1={f[1]} x2={t[0]} y2={t[1]} stroke="#1a3a5a" strokeWidth={2} />
        })}
        {/* arcs */}
        {segs.map((seg, i) => {
          if (!seg.waypoints) return null
          const color  = seg.violations > 0 ? '#ff3355' : SEGMENT_COLORS[seg.kind]
          const isSel  = selectedCrew === seg.crewId
          const pts = seg.waypoints.map(([, , ]: [number, number, number], index: number): [number, number] => {
            // Route waypoints are generated from real station positions. Use the
            // segment endpoints for SVG fallback, which remains readable on browsers
            // without WebGL while the 3-D view follows every intermediate station.
            if (index === 0) return project(seg.from)
            if (index === seg.waypoints!.length - 1) return project(seg.to)
            const first = project(seg.from)
            const last = project(seg.to)
            const t = index / (seg.waypoints!.length - 1)
            return [first[0] + (last[0] - first[0]) * t, first[1] + (last[1] - first[1]) * t]
          })
          if (pts.length < 2) return null
          const d = 'M ' + pts.map(p => `${p[0]} ${p[1]}`).join(' L ')
          return (
            <g key={i} style={{ cursor: 'pointer' }} onClick={() => onSelectCrew(isSel ? null : seg.crewId)}>
              <path d={d} fill="none" stroke={isSel ? '#fff' : color}
                strokeWidth={isSel ? 3 : 2} opacity={isSel ? 1 : 0.7} />
            </g>
          )
        })}
        {/* stations */}
        {stations.map(st => {
          const [x, y] = project(st.name)
          const color  = STATION_COLORS[st.kind] ?? '#1e3a5a'
          const r      = st.kind === 'MainDepot' ? 6 : st.kind === 'Turnaround' ? 5 : 3
          const label  = st.kind === 'MainDepot' || st.kind === 'Turnaround'
          return (
            <g key={st.id}>
              <circle cx={x} cy={y} r={r} fill={color} />
              {label && (
                <text x={x} y={y - 9} fontSize={9} fill="#c8d8ff" textAnchor="middle">{st.name}</text>
              )}
            </g>
          )
        })}
      </svg>
    </div>
  )
}
