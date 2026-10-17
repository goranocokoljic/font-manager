import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router';
import HomeView from './views/HomeView.vue';
import SettingsView from './views/SettingsView.vue';

export const routes: RouteRecordRaw[] = [
  { path: '/', name: 'home', component: HomeView },
  { path: '/settings', name: 'settings', component: SettingsView },
  { path: '/:pathMatch(.*)*', redirect: '/' },
];

// Hash history: the app is served from the Tauri asset protocol, which has no
// server to fall back to index.html for deep links.
export function createAppRouter() {
  return createRouter({ history: createWebHashHistory(), routes });
}
