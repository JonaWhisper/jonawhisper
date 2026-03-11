<script setup lang="ts">
import type { ComboboxContentProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import {
  ComboboxContent,
  ComboboxPortal,
  ComboboxViewport,
  useForwardProps,
} from "reka-ui"
import { cn } from "@/lib/utils"

defineOptions({
  inheritAttrs: false,
})

const props = withDefaults(
  defineProps<ComboboxContentProps & { class?: HTMLAttributes["class"] }>(),
  {
    position: "popper",
  },
)

const delegatedProps = reactiveOmit(props, "class")

const forwardedProps = useForwardProps(delegatedProps)
</script>

<template>
  <ComboboxPortal>
    <ComboboxContent
      v-bind="{ ...forwardedProps, ...$attrs }"
      :class="cn(
        'relative z-50 max-h-[45vh] min-w-32 overflow-hidden rounded-md border bg-popover text-popover-foreground shadow-md data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2',
        position === 'popper'
          && 'data-[side=bottom]:translate-y-1 data-[side=left]:-translate-x-1 data-[side=right]:translate-x-1 data-[side=top]:-translate-y-1',
        props.class,
      )"
    >
      <ComboboxViewport :class="cn('p-1', position === 'popper' && 'w-full min-w-[--reka-combobox-trigger-width]')">
        <slot />
      </ComboboxViewport>
    </ComboboxContent>
  </ComboboxPortal>
</template>
