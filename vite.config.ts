import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tauriConfig from './src-tauri/tauri.conf.json';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    headers: { 'Content-Security-Policy': tauriConfig.app.security.devCsp },
  },
  preview: {
    headers: { 'Content-Security-Policy': tauriConfig.app.security.csp },
  },
  build: { target: 'es2022' },
});
