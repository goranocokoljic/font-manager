import { createPinia } from 'pinia';
import { createApp } from 'vue';
import App from './App.vue';
import { useClient } from './api/client';
import { applyInterfaceSize, interfaceFactor } from './api/interfaceSize';
import { blockWebviewMenu } from './contextMenu';
import { createAppRouter } from './router';
import '@fontsource-variable/inter/opsz.css';
import './styles/theme.css';
import './styles/sidebar.css';
import './styles/print.css';
import './styles/motion.css';

// E54.1: the app shows its own menu or none; never the webview's.
blockWebviewMenu(window);
// E58.7: the interface size is on the root before the first paint, so no specimen is
// drawn at one size and then another. The shell zoomed the webview before it showed
// the window; settings that cannot be read leave the default size.
async function start(): Promise<void> {
  try {
    applyInterfaceSize(interfaceFactor(await useClient().getSettings()));
  } catch (e) {
    console.warn('the interface size was not read; the default size applies', e);
  }
  createApp(App).use(createPinia()).use(createAppRouter()).mount('#app');
}
void start();
