import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { resolve } from 'path'

export default defineConfig({
  clearScreen: false,
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  server: {
    host: process.env.TAURI_DEV_HOST || '127.0.0.1',
    port: 5183,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/target/**', '**/target/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: { target: 'esnext' },
})
