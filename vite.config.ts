import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';

const appVersion = process.env.npm_package_version ?? '0.0.0';

export default defineConfig({
  plugins: [vue()],
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    exclude: ['**/node_modules/**', '**/.worktrees/**'],
  },
});
