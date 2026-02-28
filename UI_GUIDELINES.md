# UI Guidelines

Conventions for building and maintaining the WhisperDictate interface. Follow these rules when adding or modifying any view or component.

## Design system

The app uses **shadcn-vue** (reka-ui primitives) with **Tailwind CSS**. Colors are CSS custom properties (HSL) defined in `src/assets/main.css`, mapped in `tailwind.config.ts`. Dark mode is automatic (`prefers-color-scheme`).

**Only use semantic color tokens** — never hardcode hex/rgb in HTML/CSS:

| Token | Usage |
|---|---|
| `background` / `foreground` | Page background, body text |
| `card` / `card-foreground` | Card surfaces |
| `primary` / `primary-foreground` | Primary actions, selected states |
| `muted` / `muted-foreground` | Subtle backgrounds, secondary text |
| `accent` / `accent-foreground` | Hover states, active items |
| `destructive` / `destructive-foreground` | Delete actions, error text |
| `border` | All borders |
| `input` | Input/select borders |

Exception: the FloatingPill renders on Canvas 2D where CSS variables aren't available — hardcoded colors are acceptable there.

## Spacing

### Page padding

All views use **`px-5`** horizontal padding. Vertical padding is `pt-5` / `pb-5` for the header and scrollable content areas.

### Element spacing

Only three `space-y` values:

| Value | Usage |
|---|---|
| `space-y-1` | Tight lists (sidebar nav items) |
| `space-y-2` | Between label and its control, between model cells |
| `space-y-4` | Between form field groups, between sections |

### Card/item padding

Two standards only:

| Standard | Classes | Used for |
|---|---|---|
| Normal | `px-4 py-3` | Permission cards, ModelCell rows |
| Compact | `px-3 py-2` | Model picker items (SetupStep2), history entries |

### Common margins

| Pattern | Value |
|---|---|
| Section title → content | `mb-4` |
| Search/toolbar → content | `mb-3` |
| Day label → entry list | `mb-2` |

## Components

### Buttons

Use the shadcn `Button` component. Three sizes in use:

| Size | Height | Usage |
|---|---|---|
| `default` | `h-9` | Primary CTAs (Continue, Save, Start) |
| `sm` | `h-8` | Inline secondary actions (Grant, Download) |
| `icon-sm` | `size-8` | Icon-only actions (delete on hover) |

Three variants in use:

| Variant | Usage |
|---|---|
| `default` | Primary actions (solid `bg-primary`) |
| `outline` | Secondary actions (Add server, Cancel) |
| `ghost` | Low-emphasis actions (Clear All, Copy, Delete) |

**Destructive actions**: use `variant="ghost"` with `class="text-destructive hover:text-destructive"` for secondary destructive buttons (Clear All). For confirmation dialogs, apply destructive styling on `AlertDialogAction`:

```html
<AlertDialogAction class="bg-destructive text-destructive-foreground hover:bg-destructive/90">
```

**Full-width CTAs**: add `class="w-full"` (Continue, Start, Save).

### Segmented controls (toggle buttons)

Use raw `<button>` elements inside a bordered container:

```html
<div class="inline-flex rounded-md border border-border overflow-hidden w-full">
  <button
    class="flex-1 px-3 py-1.5 text-sm transition-colors"
    :class="isActive ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50 text-muted-foreground'"
  >
```

Do NOT use `font-medium` on the active state — it makes the text wider and shifts the divider.

### Inputs and selects

Default height: **`h-9`**. Search input exception: `h-8` with icon overlay.

Search input pattern:
```html
<div class="relative px-5 mb-3">
  <Search class="absolute left-7 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground pointer-events-none" />
  <Input v-model="query" :placeholder="t('...')" class="h-8 pl-8" />
</div>
```

### Cards and list items

No shadcn `<Card>` component is used. Build cards with plain divs:

```html
<!-- Standard bordered item -->
<div class="rounded-lg border border-border bg-card px-4 py-3">

<!-- Selectable item with active state -->
<div class="rounded-lg border cursor-pointer transition-colors hover:bg-accent/30"
  :class="isSelected ? 'bg-primary/10 border-primary/30' : 'bg-card border-border'"
>

<!-- Grouped entries with dividers -->
<div class="rounded-lg border border-border divide-y divide-border">
  <div class="px-3 py-2 group">...</div>
</div>
```

### Badges

Always `variant="secondary"` with custom color override:

```html
<Badge variant="secondary" class="bg-green-500/10 text-green-500 border-transparent">
```

**Benchmark badges** use tier-based colors with dimmed values:

```html
<Badge variant="secondary"
  :class="[info.bg, 'border-transparent font-medium', 'text-[10px] px-1.5 py-0']">
  {{ info.label }} <span class="opacity-50 font-normal">{{ value }}</span>
</Badge>
```

Color tiers: `bg-emerald-500/10 text-emerald-600` (excellent), `bg-blue-500/10 text-blue-600` (good), `bg-amber-500/10 text-amber-600` (fair), `bg-orange-500/10 text-orange-600` (basic), `bg-violet-500/10 text-violet-600` (lightning).

### Download progress

Progress bars use `w-24` width. Speed text underneath in `text-[10px] text-muted-foreground`. No percentage text (the bar is sufficient).

**Optimistic pause transitions**: when the user clicks pause, immediately set `partial_progress` on the model and remove the `activeDownloads` entry in the same synchronous tick. This avoids any flash from DOM swapping between the "downloading" and "paused" template branches.

### Delete indicator

When a model is being deleted, show a greyed trash icon centered over an invisible badge (same position as the hover trash), with an indeterminate progress bar underneath:

```html
<Badge variant="secondary" class="... invisible"><!-- spacer --></Badge>
<div class="absolute inset-0 m-auto flex items-center justify-center w-8 h-8">
  <Trash2 class="w-4 h-4 text-muted-foreground/40" />
  <div class="absolute bottom-0.5 left-1 right-1 h-0.5 rounded-full overflow-hidden bg-muted-foreground/15">
    <div class="h-full w-1/3 rounded-full bg-muted-foreground/40 animate-indeterminate" />
  </div>
</div>
```

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
      <AlertDialogAction @click="doAction" class="bg-destructive text-destructive-foreground hover:bg-destructive/90">
        {{ t('...confirm') }}
      </AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
```

### Switches

Always in a flex row with label:

```html
<div class="flex items-center justify-between gap-4">
  <Label class="text-sm shrink-0">{{ t('...') }}</Label>
  <Switch :model-value="value" @update:model-value="onChange" />
</div>
```

## Layout patterns

### Two-panel (sidebar + content)

Used by Settings and ModelManager:

```
<div class="flex h-full select-none">
  <div class="w-48 min-w-[10rem] border-r border-border bg-muted/30 overflow-y-auto flex-shrink-0">
    <!-- sidebar -->
  </div>
  <div class="flex-1 min-w-0 overflow-y-auto p-5">
    <!-- content -->
  </div>
</div>
```

### Header + scrollable content

Used by History:

```
<div class="flex flex-col h-full select-none">
  <div class="px-5 pt-5 pb-2"><!-- fixed header --></div>
  <div class="flex-1 overflow-y-auto px-5 pb-5"><!-- scrollable --></div>
</div>
```

### Wizard (header + content + fixed footer)

Used by SetupWizard/SetupStep2:

```
<div class="flex flex-col h-full select-none">
  <div class="text-center px-5 pt-2 pb-3"><!-- header --></div>
  <div class="flex-1 px-5 min-h-0"><!-- flex content --></div>
  <div class="px-5 pt-3 pb-4 border-t border-border mt-2">
    <Button class="w-full">{{ t('...') }}</Button>
  </div>
</div>
```

### Scrollable flex children

Always add `min-h-0` on flex children that need to scroll — without it, they ignore the parent's height constraint:

```html
<div class="flex-1 min-h-0 overflow-y-auto">
```

## Typography

| Style | Classes | Usage |
|---|---|---|
| Page title | `text-lg font-semibold` | Section headings |
| Wizard title | `text-lg font-bold` | Setup wizard `<h1>` |
| Sidebar heading | `text-xs font-semibold uppercase tracking-wider` | Sidebar category labels |
| Form label | `text-sm font-medium` | `<Label>` elements |
| Body text | `text-sm` | Content, descriptions |
| Metadata | `text-xs text-muted-foreground` | Timestamps, secondary info |
| Sub-metadata | `text-[11px] text-muted-foreground` | Model size/benchmarks in dense lists |
| Validation error | `text-xs text-destructive` | Form field errors |

## Icons

Use **`lucide-vue-next`** exclusively. Named imports only:

```ts
import { Search, Copy, Check, Trash2 } from 'lucide-vue-next'
```

Three sizes:

| Classes | Usage |
|---|---|
| `w-4 h-4` | Standard inline icons |
| `w-3.5 h-3.5` | Compact icons (history actions, search) |
| `w-3 h-3` | Small icons (badge checkmarks) |

## i18n

- All visible text must go through `t()` (Vue) or `t!()` (Rust)
- Never hardcode user-facing strings
- Keys follow dot notation: `section.subsection.key`
- Both `en.json` and `fr.json` must stay in sync
- Don't create i18n keys speculatively — only add keys when they're actually used in a template

## Rules

1. **`h-full`, never `h-screen`** — `h-screen` overflows the Tauri webview
2. **`select-none`** on all interactive view roots — native app feel, no accidental text selection
3. **Don't call `fetchAudioDevices()` on mount** — it triggers the macOS mic permission dialog
4. **Semantic colors only** — never hardcode hex/rgb in HTML templates
5. **No `font-medium` on toggle active states** — it shifts the divider
6. **Adjust window sizes when changing padding** — SetupWizard has fixed window sizes (420x450 step 1, 680x540 step 2)
7. **Scrollable content needs `pb-5`** — bottom padding inside the scroll area so content doesn't clip against the edge
8. **Ghost buttons for secondary destructive actions** — no borders, just `text-destructive`
9. **Raw `<button>` for nav and toggles** — not `<div @click>`, for accessibility
10. **No unused i18n keys** — delete keys when removing the UI that uses them
