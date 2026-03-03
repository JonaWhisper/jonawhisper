# UI Guidelines

Conventions for building and maintaining the JonaWhisper interface. Follow these rules when adding or modifying any view or component.

## Design system

The app uses a **glassmorphism** design language inspired by native macOS panels: translucent card backgrounds, subtle blurs, thin borders, and soft shadows. Components come from **shadcn-vue** (reka-ui primitives), styled with **Tailwind CSS** for utilities and **custom CSS classes** (`.wf-*`) for the panel-specific visual layer.

Colors are CSS custom properties defined in `src/assets/main.css`. Dark mode is automatic (`prefers-color-scheme`) with full variable overrides.

### Color tokens

Two layers of tokens:

**Semantic (Tailwind / shadcn-vue)** — used in utilities and component overrides:

| Token | Usage |
|---|---|
| `background` / `foreground` | Page background, body text |
| `primary` / `primary-foreground` | Primary actions, selected states |
| `muted` / `muted-foreground` | Subtle backgrounds, secondary text |
| `accent` / `accent-foreground` | Hover states, active items |
| `destructive` / `destructive-foreground` | Delete actions, error text |
| `border` | All borders |
| `input` | Input/select borders |

**Panel (CSS custom properties)** — used in `.wf-*` classes:

| Variable | Light | Dark |
|---|---|---|
| `--panel-bg-start/end` | `#f5f5f7` / `#ebebed` | `#1c1c1e` / `#2c2c2e` |
| `--panel-card-bg` | `rgba(255,255,255,0.85)` | `rgba(44,44,46,0.85)` |
| `--panel-card-border` | `rgba(0,0,0,0.06)` | `rgba(255,255,255,0.06)` |
| `--panel-card-shadow` | `0 1px 3px rgba(0,0,0,0.06)` | `0 1px 3px rgba(0,0,0,0.2)` |
| `--panel-divider` | `rgba(0,0,0,0.06)` | `rgba(255,255,255,0.06)` |
| `--panel-accent` | `#007AFF` | `#007AFF` |
| `--sidebar-active-bg` | `rgba(0,122,255,0.10)` | `rgba(0,122,255,0.18)` |

**Never hardcode hex/rgb in templates** — use semantic tokens or panel variables.

Exception: the native pill overlay (`ui/pill.rs`) renders via RGBA buffer where CSS variables aren't available.

## Custom CSS classes

All panel-specific styling lives in `main.css` as `.wf-*` classes. **Use these instead of ad-hoc Tailwind combinations** for cards, forms, and list items.

### `.wf-card` — Card container

```css
background: var(--panel-card-bg);          /* translucent */
backdrop-filter: blur(8px);                /* glassmorphism */
border: 0.5px solid var(--panel-card-border);
border-radius: 12px;
box-shadow: var(--panel-card-shadow);
padding: 14px 16px;
margin-bottom: 10px;
```

```html
<div class="wf-card">
  <div class="wf-card-title">SECTION HEADING</div>
  <!-- content -->
</div>
```

### `.wf-card-title` — Card section heading

```css
font-size: 11px; font-weight: 600;
text-transform: uppercase; letter-spacing: 0.04em;
color: hsl(var(--muted-foreground));
margin-bottom: 10px;
```

### `.wf-form-row` — Form label + control row

```css
display: flex; align-items: center; justify-content: space-between;
padding: 8px 0; gap: 12px;
/* Adjacent rows get a 0.5px divider via + combinator */
```

```html
<div class="wf-form-row">
  <div>
    <div class="wf-form-label">Label text</div>
    <div class="wf-form-desc">Optional description</div>
  </div>
  <Select ...><!-- control --></Select>
</div>
```

### `.wf-form-label` / `.wf-form-desc`

| Class | Size | Color |
|---|---|---|
| `.wf-form-label` | 13px | `foreground` |
| `.wf-form-desc` | 11px | `muted-foreground` |

### `.wf-history-item` — History entry card

```css
display: flex; align-items: flex-start; gap: 10px;
padding: 10px 12px;
background: var(--panel-card-bg);          /* same glassmorphism */
border: 0.5px solid var(--panel-card-border);
border-radius: 10px;
margin-bottom: 6px;
/* hover: box-shadow: var(--panel-card-shadow) */
```

### `.wf-filter-chip` — Model filter pill

```css
padding: 4px 12px; border-radius: 14px; font-size: 12px;
border: 0.5px solid hsl(var(--border));
background: hsl(var(--muted));
/* Active state: add ring-1 ring-current/20 + category color via Tailwind */
```

### `.wf-provider-row` — Provider list entry

```css
display: flex; align-items: center; gap: 12px; padding: 10px 0;
/* Adjacent rows get a 0.5px divider */
```

### `.wf-about-icon` — App icon in General section

```css
width: 48px; height: 48px; border-radius: 12px;
background: linear-gradient(135deg, var(--panel-accent), #5856d6);
/* centered white text */
```

## Layout

### Panel (sidebar + content)

The main panel uses three layout classes:

```html
<div class="flex h-full select-none">
  <div class="panel-sidebar w-48 min-w-[10rem] overflow-y-auto flex-shrink-0">
    <!-- nav-pill items -->
  </div>
  <div class="panel-content flex-1 min-w-0">
    <div class="panel-content-body overflow-y-auto p-5">
      <!-- section content -->
    </div>
  </div>
</div>
```

| Class | Effect |
|---|---|
| `.panel-sidebar` | `backdrop-filter: blur(20px)`, translucent background, thin right border |
| `.panel-content` | Gradient background (`--panel-bg-start` → `--panel-bg-end`) |
| `.panel-content-body` | Custom thin scrollbar (6px, rounded thumb) |

### Section title

Every section starts with:

```html
<div class="section-title">{{ t('panel.sectionName') }}</div>
```

```css
/* .section-title */
font-size: 20px; font-weight: 700;
letter-spacing: -0.02em; margin-bottom: 16px;
```

### Nav pills (sidebar items)

```html
<button class="nav-pill" :class="{ active: isActive }">
  <Icon class="nav-icon w-4 h-4" />
  <span>Label</span>
</button>
```

```css
/* .nav-pill */
@apply rounded-lg px-2.5 py-1.5 text-sm;
/* .nav-pill.active → blue accent bg + border, font-weight: 500, icon turns --panel-accent */
```

### Status dot (sidebar footer)

```html
<span class="status-dot" :class="status" />
```

Classes: `.idle` (emerald), `.recording` (red + pulse), `.transcribing` (amber + pulse).

### Header + scrollable content

Used by History:

```html
<div class="flex flex-col h-full select-none">
  <div class="px-5 pt-5 pb-2"><!-- fixed header --></div>
  <div class="flex-1 min-h-0 overflow-y-auto px-5 pb-5"><!-- scrollable --></div>
</div>
```

### Wizard (header + content + fixed footer)

Used by SetupWizard/SetupStep2:

```html
<div class="flex flex-col h-full select-none">
  <div class="text-center px-5 pt-2 pb-3"><!-- header --></div>
  <div class="flex-1 px-5 min-h-0"><!-- content --></div>
  <div class="px-5 pt-3 pb-4 border-t border-border mt-2">
    <Button class="w-full">{{ t('...') }}</Button>
  </div>
</div>
```

### Scrollable flex children

Always `min-h-0` on flex children that need to scroll:

```html
<div class="flex-1 min-h-0 overflow-y-auto">
```

## Components (shadcn-vue)

### Buttons

Use the shadcn `Button` component. Three sizes:

| Size | Height | Usage |
|---|---|---|
| `default` | `h-9` | Primary CTAs (Continue, Save, Start) |
| `sm` | `h-8` | Inline secondary actions (Grant, Download) |
| `icon-sm` | `size-8` | Icon-only actions (delete on hover) |

Three variants:

| Variant | Usage |
|---|---|
| `default` | Primary actions (solid `bg-primary`) |
| `outline` | Secondary actions (Add server, Cancel) |
| `ghost` | Low-emphasis actions (Clear All, Copy, Delete) |

**Destructive actions**: `variant="ghost"` with `class="text-destructive hover:text-destructive"`. In confirmation dialogs:

```html
<AlertDialogAction class="bg-destructive text-destructive-foreground hover:bg-destructive/90">
```

**Full-width CTAs**: add `class="w-full"`.

### Segmented controls

Use `SegmentedToggle.vue` component, or raw `<button>` elements inside a bordered container:

```html
<div class="inline-flex rounded-md border border-border overflow-hidden w-full">
  <button class="flex-1 px-3 py-1.5 text-sm transition-colors"
    :class="isActive ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50 text-muted-foreground'">
```

Do NOT use `font-medium` on the active state — it shifts the divider.

### Selects

Default height: **`h-8 text-xs`** inside `.wf-form-row`. Search input: `h-8` with icon overlay.

**Important**: always `max-h-[45vh]` on `SelectContent` — Tauri webview is a hard physical boundary, fixed `max-h-96` can overflow when dropdown flips upward.

### Switches

Always inside a `.wf-form-row`:

```html
<div class="wf-form-row">
  <div class="wf-form-label">{{ t('...') }}</div>
  <Switch :model-value="value" @update:model-value="onChange" />
</div>
```

### Badges

Always `variant="secondary"` with color override:

```html
<Badge variant="secondary" class="bg-green-500/10 text-green-500 border-transparent">
```

**Benchmark badges** (BenchmarkBadges.vue) use tier-based colors with dimmed values:

| Tier | Background | Text |
|---|---|---|
| Excellent | `bg-emerald-500/10` | `text-emerald-600` |
| Good | `bg-blue-500/10` | `text-blue-600` |
| Fair | `bg-amber-500/10` | `text-amber-600` |
| Basic | `bg-orange-500/10` | `text-orange-600` |
| Lightning | `bg-violet-500/10` | `text-violet-600` |

### Alert dialogs

Always controlled externally (no `AlertDialogTrigger`):

```html
<AlertDialog :open="showConfirm" @update:open="showConfirm = $event">
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>{{ t('...') }}</AlertDialogTitle>
      <AlertDialogDescription>{{ t('...') }}</AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel @click="showConfirm = false">{{ t('...cancel') }}</AlertDialogCancel>
      <AlertDialogAction @click="doAction" class="bg-destructive ...">
        {{ t('...confirm') }}
      </AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
```

### Download progress

Progress bars: `w-24`. Speed text: `text-[10px] text-muted-foreground`. No percentage text.

**Optimistic pause transitions**: when the user clicks pause, immediately set `partial_progress` and remove the `activeDownloads` entry in the same synchronous tick to avoid flash.

### Delete indicator

Greyed trash icon + indeterminate progress bar while deleting:

```html
<div class="absolute inset-0 m-auto flex items-center justify-center w-8 h-8">
  <Trash2 class="w-4 h-4 text-muted-foreground/40" />
  <div class="absolute bottom-0.5 left-1 right-1 h-0.5 rounded-full overflow-hidden bg-muted-foreground/15">
    <div class="h-full w-1/3 rounded-full bg-muted-foreground/40 animate-indeterminate" />
  </div>
</div>
```

### Tooltips

Use shadcn-vue `Tooltip` with `delay-duration="300"` everywhere instead of native `title` attribute:

```html
<TooltipProvider>
  <Tooltip :delay-duration="300">
    <TooltipTrigger as-child>
      <Button variant="ghost" size="icon-sm">...</Button>
    </TooltipTrigger>
    <TooltipContent>{{ t('...') }}</TooltipContent>
  </Tooltip>
</TooltipProvider>
```

## Typography

| Style | Definition | Usage |
|---|---|---|
| Section title | `.section-title` (20px, bold, -0.02em tracking) | Panel section headings |
| Card heading | `.wf-card-title` (11px, semibold, uppercase, 0.04em tracking) | Card section labels |
| Form label | `.wf-form-label` (13px, foreground) | Setting labels |
| Form description | `.wf-form-desc` (11px, muted-foreground) | Setting descriptions |
| Wizard title | `text-lg font-bold` | Setup wizard `<h1>` |
| Nav item | `text-sm` (via `.nav-pill`) | Sidebar items |
| Body text | `text-sm` | General content |
| Metadata | `text-xs text-muted-foreground` | Timestamps, secondary info |
| Sub-metadata | `text-[11px] text-muted-foreground` | Model sizes in dense lists |
| Badge text | `text-[10px]` | Benchmark badges |
| Validation error | `text-xs text-destructive` | Form field errors |

## Icons

Use **`lucide-vue-next`** exclusively. Named imports only:

```ts
import { Search, Copy, Check, Trash2 } from 'lucide-vue-next'
```

Four sizes:

| Classes | Usage |
|---|---|
| `w-5 h-5` | Large / hero icons |
| `w-4 h-4` | Standard inline icons, nav icons |
| `w-3.5 h-3.5` | Compact icons (history actions, search) |
| `w-3 h-3` / `w-2.5 h-2.5` | Small icons (badge checkmarks, indicators) |

## Animations

| Name | Duration | Used for |
|---|---|---|
| `.fade-enter/leave` | 200ms ease | Section transitions (Panel tabs) |
| `.status-pulse` | 1.2s infinite | Recording/transcribing status dot |
| `.captureFlash` | 1s infinite | Shortcut capture waiting hint |
| `.animate-indeterminate` | 1.5s infinite | Model delete progress bar |
| `transition-[height] duration-75` | 75ms | Spectrum bar height changes |

## i18n

- All visible text through `t()` (Vue) or `t!()` (Rust)
- Never hardcode user-facing strings
- Keys follow dot notation: `section.subsection.key`
- Both `en.json` and `fr.json` must stay in sync
- Don't create i18n keys speculatively — only add when actually used

## Rules

1. **`h-full`, never `h-screen`** — `h-screen` overflows the Tauri webview
2. **`select-none`** on all interactive view roots — native app feel
3. **Don't call `fetchAudioDevices()` on mount** — triggers macOS mic permission dialog
4. **Semantic colors only** — never hardcode hex/rgb in templates
5. **Use `.wf-*` classes for cards and forms** — not ad-hoc Tailwind combinations
6. **No `font-medium` on toggle active states** — shifts the divider
7. **Adjust window sizes when changing padding** — SetupWizard has fixed sizes (420x450 step 1, 680x540 step 2)
8. **Scrollable content needs `pb-5`** — bottom padding so content doesn't clip
9. **Ghost buttons for secondary destructive actions** — no borders, just `text-destructive`
10. **Raw `<button>` for nav and toggles** — not `<div @click>`, for accessibility
11. **No unused i18n keys** — delete keys when removing the UI that uses them
12. **`max-h-[45vh]` on SelectContent** — prevents overflow in Tauri webview
