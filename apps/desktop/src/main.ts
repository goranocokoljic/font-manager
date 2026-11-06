import { createPinia } from 'pinia';
import { createApp } from 'vue';
import App from './App.vue';
import { createAppRouter } from './router';
import '@fontsource-variable/inter/opsz.css';
import './styles/theme.css';
import './styles/sidebar.css';
import './styles/print.css';

createApp(App).use(createPinia()).use(createAppRouter()).mount('#app');
