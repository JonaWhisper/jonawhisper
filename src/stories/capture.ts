import { chromium, type Page } from 'playwright'
import { spawn, type ChildProcess } from 'child_process'
import { mkdirSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const PROJECT_ROOT = resolve(__dirname, '../..')
const SCREENSHOTS_DIR = resolve(PROJECT_ROOT, 'docs/screenshots')
const HISTOIRE_PORT = 6006
const BASE_URL = `http://127.0.0.1:${HISTOIRE_PORT}`
const STORY_ID = 'src-stories-uipatterns-story-vue'

const VARIANTS = [
  'card',
  'form-rows',
  'section-title',
  'filter-chips',
  'history-item',
  'nav-pills',
  'status-dots',
  'provider-row',
  'about-icon',
  'day-group',
]

async function waitForServer(url: string, timeout = 60_000): Promise<void> {
  const start = Date.now()
  while (Date.now() - start < timeout) {
    try {
      const res = await fetch(url)
      if (res.ok) return
    } catch {
      // not ready yet
    }
    await new Promise((r) => setTimeout(r, 500))
  }
  throw new Error(`Server at ${url} did not start within ${timeout}ms`)
}

async function waitForApp(page: Page): Promise<void> {
  await page.waitForFunction(
    () => {
      const app = document.getElementById('app')
      if (!app) return false
      // Check that app has rendered content (not just the hidden mount div)
      return app.querySelector('.htw-sandbox-hidden + *') !== null
    },
    null,
    { timeout: 30_000 },
  )
}

async function captureVariant(
  page: Page,
  variantId: string,
  mode: 'light' | 'dark',
): Promise<void> {
  const url = `${BASE_URL}/__sandbox?storyId=${STORY_ID}&variantId=${variantId}`
  await page.goto(url, { waitUntil: 'networkidle' })
  await waitForApp(page)

  // Toggle dark mode via class on <html>
  if (mode === 'dark') {
    await page.evaluate(() => document.documentElement.classList.add('dark'))
  } else {
    await page.evaluate(() => document.documentElement.classList.remove('dark'))
  }

  // Wait for styles to settle
  await page.waitForTimeout(300)

  const filename = `pattern-${variantId}-${mode}.png`
  await page.screenshot({
    path: resolve(SCREENSHOTS_DIR, filename),
    fullPage: true,
  })
  console.log(`  captured ${filename}`)
}

async function main(): Promise<void> {
  mkdirSync(SCREENSHOTS_DIR, { recursive: true })

  console.log('Starting Histoire dev server...')
  const server: ChildProcess = spawn('npx', ['histoire', 'dev', '--port', String(HISTOIRE_PORT)], {
    stdio: 'pipe',
    cwd: PROJECT_ROOT,
  })

  let serverOutput = ''
  server.stdout?.on('data', (d: Buffer) => { serverOutput += d.toString() })
  server.stderr?.on('data', (d: Buffer) => { serverOutput += d.toString() })

  try {
    await waitForServer(BASE_URL)
    console.log('Histoire server ready.')

    const browser = await chromium.launch()
    const context = await browser.newContext({ viewport: { width: 800, height: 600 } })
    const page = await context.newPage()

    // Warmup: load the main page first so Vite compiles all modules
    console.log('Warming up Vite...')
    await page.goto(`${BASE_URL}/__sandbox?storyId=${STORY_ID}&variantId=card`, {
      waitUntil: 'networkidle',
    })
    await waitForApp(page)
    console.log('Warmup done.')

    for (const variant of VARIANTS) {
      for (const mode of ['light', 'dark'] as const) {
        await captureVariant(page, variant, mode)
      }
    }

    await browser.close()
    console.log(`\nDone! ${VARIANTS.length * 2} screenshots saved to docs/screenshots/`)
  } catch (err) {
    console.error('Error:', err)
    console.error('Server output:', serverOutput)
    process.exit(1)
  } finally {
    server.kill()
  }
}

main()
