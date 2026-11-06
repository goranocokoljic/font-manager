import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vitest/config';

// Tauri expects a fixed port and must see Rust errors in the terminal.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    // The contrast test reads the theme's and the print palette's colors (`?raw`).
    css: { include: [/(theme|print)\.css/] },
    // A CI runner can stall for about 10 s under the parallel load. No test here asserts its time.
    testTimeout: 30_000,
  },
});
