import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/panel' },
    { path: '/panel', component: () => import('./views/Panel.vue') },
    { path: '/setup', component: () => import('./views/SetupWizard.vue') },
  ],
})

export default router
