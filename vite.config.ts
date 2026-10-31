import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  resolve: {
    // Exact-match so the shim's own `@tauri-apps/api` import doesn't recurse.
    // Every `import { invoke } from '@tauri-apps/api/core'` — ours and the
    // plugins' — lands on the shim, which is why the shim has to re-export
    // everything the plugins pull from that specifier (see src/lib/tauri-core.ts).
    alias: [
      {
        find: /^@tauri-apps\/api\/core$/,
        replacement: fileURLToPath(new URL('./src/lib/tauri-core.ts', import.meta.url)),
      },
    ],
  },
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1431 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id: string) {
          if (id.includes('node_modules/react/') || id.includes('node_modules/react-dom/')) {
            return 'react-vendor';
          }
          if (id.includes('@tanstack')) return 'tanstack';
          if (id.includes('zustand')) return 'state';
          if (id.includes('@tauri-apps')) return 'tauri';
        },
      },
    },
  },
}));
