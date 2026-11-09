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
    // The contrast test reads the theme's and the print palette's colors, and the motion test
    // every stylesheet of src/styles (`?raw`); a stylesheet left out reads as empty.
    css: { include: [/src[\\/]styles[\\/][\w-]+\.css/] },
    // A CI runner can stall for about 10 s under the parallel load. No test here asserts its time.
    testTimeout: 30_000,
  },
});
