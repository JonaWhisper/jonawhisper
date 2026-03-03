import { defineConfig } from 'histoire'
import { HstVue } from '@histoire/plugin-vue'

export default defineConfig({
  plugins: [HstVue()],
  setupFile: '/src/stories/setup.ts',
  storyMatch: ['src/stories/**/*.story.vue'],
  storyIgnored: ['**/node_modules/**', '**/src-tauri/**', '**/build/**'],
  theme: {
    title: 'JonaWhisper UI',
  },
  vite: {
    resolve: {
      alias: {
        '@': new URL('./src', import.meta.url).pathname,
      },
    },
    server: {
      host: '127.0.0.1',
      watch: {
        ignored: ['**/src-tauri/**', '**/build/**', '**/.git/**'],
      },
    },
  },
})
