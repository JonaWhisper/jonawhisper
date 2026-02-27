<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed } from 'vue'
import { emit } from '@tauri-apps/api/event'
import { useAppStore } from '../stores/app'

const store = useAppStore()
const canvas = ref<HTMLCanvasElement | null>(null)
let animFrame = 0
let dotPhase = 0

type PillMode = 'recording' | 'transcribing' | 'downloading' | 'error' | 'idle'

const mode = computed<PillMode>(() => store.pillMode)

const smoothedSpectrum = ref<number[]>(new Array(12).fill(0))

watch(() => store.spectrumData, (newData) => {
  if (!newData || newData.length === 0) return
  const smoothed = [...smoothedSpectrum.value]
  for (let i = 0; i < smoothed.length; i++) {
    const newVal = i < newData.length ? (newData[i] ?? 0) : 0
    smoothed[i] = (smoothed[i] ?? 0) * 0.45 + newVal * 0.55
  }
  smoothedSpectrum.value = smoothed
})

function draw() {
  const c = canvas.value
  if (!c) return
  const ctx = c.getContext('2d')
  if (!ctx) return

  const dpr = window.devicePixelRatio || 1
  c.width = 140 * dpr
  c.height = 56 * dpr
  c.style.width = '140px'
  c.style.height = '56px'
  ctx.scale(dpr, dpr)

  const cw = 140
  const ch = 56

  // Clear
  ctx.clearRect(0, 0, cw, ch)

  // Draw pill background
  const radius = ch / 2
  ctx.beginPath()
  ctx.roundRect(0, 0, cw, ch, radius)
  ctx.fillStyle = 'rgba(30, 30, 30, 0.9)'
  ctx.fill()

  // Draw content based on mode
  const m = mode.value

  if (m === 'recording') {
    drawSpectrum(ctx, cw, ch)
  } else if (m === 'transcribing') {
    drawDots(ctx, cw, ch)
  } else if (m === 'downloading') {
    drawProgress(ctx, cw, ch)
  } else if (m === 'error') {
    drawError(ctx, cw, ch)
  }

  // Queue badge — show total pending (queued + currently transcribing)
  const pending = store.queueCount + (store.isTranscribing ? 1 : 0)
  if (pending > 1) {
    const badgeSize = 16
    const bx = cw - badgeSize / 2 - 2
    const by = badgeSize / 2 + 2
    ctx.beginPath()
    ctx.arc(bx, by, badgeSize / 2, 0, Math.PI * 2)
    ctx.fillStyle = '#ef4444'
    ctx.fill()
    ctx.fillStyle = '#fff'
    ctx.font = 'bold 10px -apple-system, sans-serif'
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText(String(pending), bx, by)
  }

  animFrame = requestAnimationFrame(draw)
}

function drawSpectrum(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const bars = smoothedSpectrum.value
  const barCount = bars.length
  const barWidth = 4
  const gap = 3
  const totalWidth = barCount * barWidth + (barCount - 1) * gap
  const startX = (w - totalWidth) / 2
  const maxHeight = h * 0.6
  const centerY = h / 2

  for (let i = 0; i < barCount; i++) {
    const barHeight = Math.max(2, (bars[i] ?? 0) * maxHeight)
    const x = startX + i * (barWidth + gap)
    const y = centerY - barHeight / 2

    ctx.beginPath()
    ctx.roundRect(x, y, barWidth, barHeight, barWidth / 2)
    ctx.fillStyle = '#ffffff'
    ctx.fill()
  }
}

function drawDots(ctx: CanvasRenderingContext2D, w: number, h: number) {
  dotPhase += 0.05
  const dotCount = 3
  const dotSize = 5
  const gap = 10
  const totalWidth = dotCount * dotSize + (dotCount - 1) * gap
  const startX = (w - totalWidth) / 2

  for (let i = 0; i < dotCount; i++) {
    const phase = dotPhase + i * 0.8
    const bounce = Math.sin(phase) * 0.3 + 0.7
    const x = startX + i * (dotSize + gap) + dotSize / 2
    const y = h / 2

    ctx.beginPath()
    ctx.arc(x, y, (dotSize / 2) * bounce, 0, Math.PI * 2)
    ctx.fillStyle = `rgba(255, 255, 255, ${0.4 + bounce * 0.6})`
    ctx.fill()
  }
}

function drawProgress(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const barWidth = w * 0.7
  const barHeight = 4
  const x = (w - barWidth) / 2
  const y = h / 2 - barHeight / 2

  // Background
  ctx.beginPath()
  ctx.roundRect(x, y, barWidth, barHeight, barHeight / 2)
  ctx.fillStyle = 'rgba(255, 255, 255, 0.2)'
  ctx.fill()

  // Progress
  const progress = store.downloadProgress
  if (progress > 0) {
    ctx.beginPath()
    ctx.roundRect(x, y, barWidth * progress, barHeight, barHeight / 2)
    ctx.fillStyle = '#ffffff'
    ctx.fill()
  }
}

function drawError(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const size = 16
  const cx = w / 2
  const cy = h / 2

  ctx.strokeStyle = '#ef4444'
  ctx.lineWidth = 3
  ctx.lineCap = 'round'

  ctx.beginPath()
  ctx.moveTo(cx - size / 2, cy - size / 2)
  ctx.lineTo(cx + size / 2, cy + size / 2)
  ctx.stroke()

  ctx.beginPath()
  ctx.moveTo(cx + size / 2, cy - size / 2)
  ctx.lineTo(cx - size / 2, cy + size / 2)
  ctx.stroke()
}

onMounted(async () => {
  // Ensure transparent background for pill window
  document.documentElement.style.background = 'transparent'
  document.body.style.background = 'transparent'
  document.body.style.margin = '0'
  document.body.style.overflow = 'hidden'
  animFrame = requestAnimationFrame(draw)
  // Signal to Rust that the webview is ready to be shown
  await emit('pill-ready')
})

onUnmounted(() => {
  cancelAnimationFrame(animFrame)
})
</script>

<template>
  <div class="pill-window flex items-center justify-center w-full h-full">
    <canvas ref="canvas" width="140" height="56" />
  </div>
</template>

<style scoped>
.pill-window {
  background: transparent;
  -webkit-app-region: drag;
}
</style>
