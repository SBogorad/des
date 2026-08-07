import { readFileSync } from 'node:fs'

const base = process.env.BASE_URL ?? 'http://127.0.0.1:4173'
const paths = [
  '/?crew=CREW-0004&mode=presentation',
  '/findings.json',
  '/network.json',
  '/sim.json',
]

for (const path of paths) {
  const response = await fetch(`${base}${path}`)
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`)
  const body = await response.text()
  if (body.length < 100) throw new Error(`${path}: response is unexpectedly small`)
  console.log(`${response.status} ${path} ${body.length} bytes`)
}

const findings = JSON.parse(await (await fetch(`${base}/findings.json`)).text())
if (!findings.some(finding => finding.crewId === 'CREW-0004' && finding.severity === 'critical')) {
  throw new Error('Critical CREW-0004 finding is missing')
}

const html = await (await fetch(`${base}/?crew=CREW-0004&mode=presentation`)).text()
if (!html.includes('/assets/index-') || !html.includes('type="module"')) {
  throw new Error('Built HTML does not reference the application module')
}

console.log('Visualization smoke check passed.')
