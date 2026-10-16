import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vitest/config';

// Tauri expects a fixed port and must see Rust errors in the terminal.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  test: { environment: 'jsdom' },
});
