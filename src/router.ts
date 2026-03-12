import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/panel' },
    { path: '/panel', component: () => import('./views/Panel.vue') },
    { path: '/setup', component: () => import('./views/SetupWizard.vue') },
    { path: '/provider-form', component: () => import('./views/ProviderFormView.vue') },
  ],
})

export default router
