import { readdirSync, readFileSync, statSync } from 'node:fs'
import { resolve } from 'node:path'
import { gzipSync } from 'node:zlib'

const distDir = resolve(process.env.PERFORMANCE_DIST_DIR ?? 'dist')
const assetDir = resolve(distDir, 'assets')
const files = readdirSync(assetDir)
  .filter(name => name.endsWith('.js'))
  .map(name => {
    const contents = readFileSync(resolve(assetDir, name))
    return { name, rawBytes: contents.length, gzipBytes: gzipSync(contents).length }
  })

const failures = []
const requireBudget = (condition, message) => {
  if (!condition) failures.push(message)
}
const threeChunk = files.find(file => file.name.startsWith('three-vendor-'))
const initialChunks = files.filter(file => file.name.startsWith('index-') || file.name.startsWith('react-vendor-'))
const initialGzipBytes = initialChunks.reduce((total, file) => total + file.gzipBytes, 0)
const totalGzipBytes = files.reduce((total, file) => total + file.gzipBytes, 0)

requireBudget(statSync(resolve(distDir, 'index.html')).size <= 16 * 1024, 'index.html exceeds 16 KiB')
requireBudget(threeChunk, 'missing named three-vendor chunk')
requireBudget(threeChunk?.gzipBytes <= 384 * 1024, 'three-vendor gzip size exceeds 384 KiB')
requireBudget(initialGzipBytes <= 160 * 1024, 'initial JavaScript gzip size exceeds 160 KiB')
requireBudget(totalGzipBytes <= 512 * 1024, 'total JavaScript gzip size exceeds 512 KiB')

for (const file of files.sort((left, right) => right.gzipBytes - left.gzipBytes)) {
  console.log(`${file.name}: ${file.rawBytes} raw bytes, ${file.gzipBytes} gzip bytes`)
}
console.log(`Initial JS gzip bytes: ${initialGzipBytes}`)
console.log(`Total JS gzip bytes: ${totalGzipBytes}`)

if (failures.length > 0) {
  for (const failure of failures) console.error(`Performance budget failed: ${failure}`)
  process.exit(1)
}
