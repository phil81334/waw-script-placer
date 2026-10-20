import { defineConfig } from 'vite';

export default defineConfig({
  clearScreen: false,
  // The desktop app expects the same fixed address in tauri.conf.json.
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**', '**/.local/**'] },
  },
  build: { target: 'es2022', assetsInlineLimit: 0 },
});
