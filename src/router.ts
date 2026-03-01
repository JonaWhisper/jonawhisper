import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/settings' },
    { path: '/model-manager', component: () => import('./views/ModelManager.vue') },
    { path: '/setup', component: () => import('./views/SetupWizard.vue') },
    { path: '/settings', component: () => import('./views/Settings.vue') },
    { path: '/history', component: () => import('./views/History.vue') },
  ],
})

export default router
