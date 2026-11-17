import { createPinia } from 'pinia';
import { createApp } from 'vue';
import App from './App.vue';
import { blockWebviewMenu } from './contextMenu';
import { createAppRouter } from './router';
import '@fontsource-variable/inter/opsz.css';
import './styles/theme.css';
import './styles/sidebar.css';
import './styles/print.css';
import './styles/motion.css';

// E54.1: the app shows its own menu or none; never the webview's.
blockWebviewMenu(window);
createApp(App).use(createPinia()).use(createAppRouter()).mount('#app');
