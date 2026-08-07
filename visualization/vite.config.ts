import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Set BASE to the public mount path when deploying outside the root.
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
