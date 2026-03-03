<script setup lang="ts">
import { Switch } from '@/components/ui/switch'
import { Badge } from '@/components/ui/badge'
import {
  Settings, History, Cpu, Mic, Wand2, Globe, Brain, Trash2,
} from 'lucide-vue-next'
</script>

<template>
  <Story title="UI Patterns" auto-props-disabled>
    <!-- Card -->
    <Variant title="Card" id="card">
      <div class="p-6 bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))] min-h-[200px]">
        <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
          <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">
            TRANSCRIPTION
          </div>
          <div class="flex items-center justify-between py-2 gap-3">
            <div>
              <div class="text-[13px] text-foreground">Auto-detect language</div>
              <div class="text-[11px] text-muted-foreground mt-px">Detect spoken language automatically</div>
            </div>
            <Switch :model-value="true" />
          </div>
          <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
            <div>
              <div class="text-[13px] text-foreground">Model</div>
            </div>
            <div class="h-8 rounded-md border border-input px-3 text-xs text-muted-foreground flex items-center">
              Whisper Large v3
            </div>
          </div>
        </div>
      </div>
    </Variant>

    <!-- Form Rows -->
    <Variant title="Form Rows" id="form-rows">
      <div class="p-6 bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))]">
        <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px]">
          <div class="flex items-center justify-between py-2 gap-3">
            <div>
              <div class="text-[13px] text-foreground">Voice Activity Detection</div>
              <div class="text-[11px] text-muted-foreground mt-px">Trim silence before transcription</div>
            </div>
            <Switch :model-value="true" />
          </div>
          <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
            <div>
              <div class="text-[13px] text-foreground">Punctuation restoration</div>
              <div class="text-[11px] text-muted-foreground mt-px">Add punctuation to raw ASR output</div>
            </div>
            <Switch :model-value="false" />
          </div>
          <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
            <div>
              <div class="text-[13px] text-foreground">Launch at login</div>
            </div>
            <Switch :model-value="true" />
          </div>
        </div>
      </div>
    </Variant>

    <!-- Section Title -->
    <Variant title="Section Title" id="section-title">
      <div class="p-6 bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))]">
        <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">Settings</div>
        <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">History</div>
        <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">Models</div>
      </div>
    </Variant>

    <!-- Filter Chips -->
    <Variant title="Filter Chips" id="filter-chips">
      <div class="p-6 bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))]">
        <div class="flex flex-wrap gap-1.5">
          <button class="px-3 py-1 rounded-[14px] text-xs cursor-pointer border-[0.5px] transition-all duration-150 font-[inherit] inline-flex items-center gap-1.5 bg-blue-500/10 text-blue-500 border-transparent ring-1 ring-current/20">
            <Mic class="w-3 h-3" /> ASR
          </button>
          <button class="px-3 py-1 rounded-[14px] text-xs cursor-pointer border-[0.5px] border-border transition-all duration-150 font-[inherit] inline-flex items-center gap-1.5 bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground">
            <Wand2 class="w-3 h-3" /> Punctuation
          </button>
          <button class="px-3 py-1 rounded-[14px] text-xs cursor-pointer border-[0.5px] border-border transition-all duration-150 font-[inherit] inline-flex items-center gap-1.5 bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground">
            <Globe class="w-3 h-3" /> Correction
          </button>
          <button class="px-3 py-1 rounded-[14px] text-xs cursor-pointer border-[0.5px] border-border transition-all duration-150 font-[inherit] inline-flex items-center gap-1.5 bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground">
            <Brain class="w-3 h-3" /> LLM
          </button>
          <button class="px-3 py-1 rounded-[14px] text-xs cursor-pointer border-[0.5px] border-border transition-all duration-150 font-[inherit] inline-flex items-center gap-1.5 bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground">
            <Cpu class="w-3 h-3" /> All
          </button>
        </div>
      </div>
    </Variant>

    <!-- History Item -->
    <Variant title="History Item" id="history-item">
      <div class="p-6 bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))]">
        <div class="flex items-start gap-2.5 p-[10px_12px] bg-panel-card-bg border-[0.5px] border-panel-card-border rounded-[10px] mb-1.5 transition-shadow duration-150 hover:shadow-panel-card group">
          <div class="flex-1 min-w-0">
            <div class="text-xs text-muted-foreground mb-1">14:32 &middot; 3.2s &middot; Whisper Large v3</div>
            <div class="text-sm text-foreground line-clamp-2">
              Bonjour, ceci est un exemple de transcription vocale avec ponctuation et correction automatique.
            </div>
            <div class="flex gap-1 mt-1.5">
              <Badge variant="secondary" class="bg-green-500/10 text-green-500 border-transparent text-[10px]">
                VAD
              </Badge>
              <Badge variant="secondary" class="bg-blue-500/10 text-blue-500 border-transparent text-[10px]">
                BERT
              </Badge>
            </div>
          </div>
        </div>
      </div>
    </Variant>

    <!-- Nav Pills -->
    <Variant title="Nav Pills" id="nav-pills">
      <div class="w-44 backdrop-blur-[20px] backdrop-saturate-[1.8] bg-[hsl(var(--background)/0.72)] dark:bg-[hsl(var(--background)/0.65)] border-r-[0.5px] border-[hsl(var(--border)/0.5)] p-2 flex flex-col gap-0.5">
        <button class="rounded-lg px-2.5 py-1.5 text-sm transition-all border w-full text-left flex items-center gap-2 bg-sidebar-active-bg border-sidebar-active-border font-medium">
          <Settings class="w-[18px] h-[18px] opacity-100 text-panel-accent" />
          <span>Settings</span>
        </button>
        <button class="rounded-lg px-2.5 py-1.5 text-sm transition-all border border-transparent hover:bg-sidebar-hover-bg w-full text-left flex items-center gap-2">
          <History class="w-[18px] h-[18px] opacity-70" />
          <span>History</span>
        </button>
        <button class="rounded-lg px-2.5 py-1.5 text-sm transition-all border border-transparent hover:bg-sidebar-hover-bg w-full text-left flex items-center gap-2">
          <Cpu class="w-[18px] h-[18px] opacity-70" />
          <span>Models</span>
        </button>
      </div>
    </Variant>

    <!-- Status Dots -->
    <Variant title="Status Dots" id="status-dots">
      <div class="p-6 bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))] flex items-center gap-6">
        <div class="flex items-center gap-2">
          <span class="inline-block w-2 h-2 rounded-full bg-emerald-500" />
          <span class="text-xs text-muted-foreground">Idle</span>
        </div>
        <div class="flex items-center gap-2">
          <span class="inline-block w-2 h-2 rounded-full bg-red-500 animate-status-pulse" />
          <span class="text-xs text-muted-foreground">Recording</span>
        </div>
        <div class="flex items-center gap-2">
          <span class="inline-block w-2 h-2 rounded-full bg-amber-500 animate-status-pulse" />
          <span class="text-xs text-muted-foreground">Transcribing</span>
        </div>
      </div>
    </Variant>

    <!-- Provider Row -->
    <Variant title="Provider Row" id="provider-row">
      <div class="p-6 bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))]">
        <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px]">
          <div class="flex items-center gap-3 py-2.5">
            <div class="w-8 h-8 rounded-lg bg-gradient-to-br from-green-500 to-emerald-600 flex items-center justify-center text-white text-sm font-bold">
              W
            </div>
            <div class="flex-1 min-w-0">
              <div class="text-[13px] text-foreground">Whisper</div>
              <div class="text-[11px] text-muted-foreground">OpenAI</div>
            </div>
            <Badge variant="secondary" class="bg-blue-500/10 text-blue-500 border-transparent text-[10px]">
              Local
            </Badge>
          </div>
          <div class="flex items-center gap-3 py-2.5 border-t-[0.5px] border-panel-divider">
            <div class="w-8 h-8 rounded-lg bg-gradient-to-br from-violet-500 to-purple-600 flex items-center justify-center text-white text-sm font-bold">
              C
            </div>
            <div class="flex-1 min-w-0">
              <div class="text-[13px] text-foreground">Canary</div>
              <div class="text-[11px] text-muted-foreground">NVIDIA</div>
            </div>
            <Badge variant="secondary" class="bg-blue-500/10 text-blue-500 border-transparent text-[10px]">
              Local
            </Badge>
          </div>
        </div>
      </div>
    </Variant>

    <!-- About Icon -->
    <Variant title="About Icon" id="about-icon">
      <div class="p-6 bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))] text-center">
        <div class="w-12 h-12 mx-auto mb-2 bg-gradient-to-br from-panel-accent to-[#5856d6] rounded-xl flex items-center justify-center text-[22px] font-bold text-white">
          J
        </div>
        <div class="text-sm font-semibold text-foreground">JonaWhisper</div>
        <div class="text-xs text-muted-foreground">v0.1.0</div>
      </div>
    </Variant>

    <!-- Day Group -->
    <Variant title="Day Group" id="day-group">
      <div class="p-6 bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))]">
        <div class="group/day">
          <div class="flex items-center justify-between mb-2">
            <div class="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
              Today
            </div>
            <button class="opacity-0 group-hover/day:opacity-100 transition-opacity duration-150 p-1 rounded hover:bg-accent">
              <Trash2 class="w-3.5 h-3.5 text-destructive" />
            </button>
          </div>
          <div class="flex items-start gap-2.5 p-[10px_12px] bg-panel-card-bg border-[0.5px] border-panel-card-border rounded-[10px] mb-1.5">
            <div class="flex-1 min-w-0">
              <div class="text-xs text-muted-foreground mb-1">09:15 &middot; 2.1s</div>
              <div class="text-sm text-foreground line-clamp-2">First transcription of the day.</div>
            </div>
          </div>
          <div class="flex items-start gap-2.5 p-[10px_12px] bg-panel-card-bg border-[0.5px] border-panel-card-border rounded-[10px] mb-1.5">
            <div class="flex-1 min-w-0">
              <div class="text-xs text-muted-foreground mb-1">09:42 &middot; 4.8s</div>
              <div class="text-sm text-foreground line-clamp-2">Another entry with longer text content to show truncation behavior.</div>
            </div>
          </div>
        </div>
        <!-- Simulate hover state -->
        <div class="mt-4">
          <div class="flex items-center justify-between mb-2">
            <div class="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
              Yesterday (hover simulated)
            </div>
            <button class="opacity-100 transition-opacity duration-150 p-1 rounded hover:bg-accent">
              <Trash2 class="w-3.5 h-3.5 text-destructive" />
            </button>
          </div>
          <div class="flex items-start gap-2.5 p-[10px_12px] bg-panel-card-bg border-[0.5px] border-panel-card-border rounded-[10px] mb-1.5">
            <div class="flex-1 min-w-0">
              <div class="text-xs text-muted-foreground mb-1">17:30 &middot; 1.5s</div>
              <div class="text-sm text-foreground line-clamp-2">Yesterday's entry.</div>
            </div>
          </div>
        </div>
      </div>
    </Variant>
  </Story>
</template>
