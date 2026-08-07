import { cpSync, mkdirSync, rmSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const projectDir = fileURLToPath(new URL('.', import.meta.url))
const distDir = resolve(projectDir, 'dist')
const node = process.execPath

function run(args, env = {}) {
  const result = spawnSync(node, args, {
    cwd: projectDir,
    env: { ...process.env, ...env },
    stdio: 'inherit',
  })
  if (result.status !== 0) process.exit(result.status ?? 1)
}

rmSync(distDir, { recursive: true, force: true })
mkdirSync(distDir, { recursive: true })

run(['generate-findings.mjs'])
run(['node_modules/typescript/bin/tsc'])
run(['node_modules/vite/bin/vite.js', 'build'], { BASE: '/demo/', OUT_DIR: 'dist/demo' })

cpSync(resolve(projectDir, 'landing/index.html'), resolve(distDir, 'index.html'))
cpSync(resolve(projectDir, 'landing/styles.css'), resolve(distDir, 'styles.css'))
console.log('Built DES landing and interactive demo.')
