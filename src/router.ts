import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/pill' },
    { path: '/pill', component: () => import('./views/FloatingPill.vue') },
    { path: '/model-manager', component: () => import('./views/ModelManager.vue') },
    { path: '/setup', component: () => import('./views/SetupWizard.vue') },
    { path: '/settings', component: () => import('./views/Settings.vue') },
  ],
})

export default router
