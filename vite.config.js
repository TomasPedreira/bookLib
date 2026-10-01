import { defineConfig } from 'vite';

export default defineConfig({
  root: 'web',
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: process.env.TAURI_DEV_HOST || '0.0.0.0',
    hmr: process.env.TAURI_DEV_HOST ? { host: process.env.TAURI_DEV_HOST, port: 1421 } : undefined,
  },
  build: { outDir: '../dist', emptyOutDir: true, target: 'es2020' },
});
