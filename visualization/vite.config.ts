import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// When deployed at https://ced.syntog.ru/viz/, set BASE=/viz/ in the environment.
// For local dev (vite) the default '/' keeps HMR working normally.
const base = process.env.BASE ?? '/'
const outDir = process.env.OUT_DIR ?? 'dist'

export default defineConfig({
  plugins: [react()],
  base,
  build: {
    outDir,
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks: {
          'react-vendor': ['react', 'react-dom'],
          'three-vendor': ['three', '@react-three/fiber', '@react-three/drei'],
        },
      },
    },
  },
})
