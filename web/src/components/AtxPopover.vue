<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Power, RotateCcw, CircleDot, Wifi, Send, HardDrive, Check } from 'lucide-vue-next'
import { atxApi } from '@/api'
import type { WolTarget } from '@/types/generated'

type AtxAction = 'short' | 'long' | 'reset'

const minActionFeedbackMs = 800
const actionDurations: Record<AtxAction, number> = {
  short: 500,
  long: 5000,
  reset: 500,
}

const props = withDefaults(defineProps<{
  /** ATX hardware power control is configured and enabled */
  atxAvailable?: boolean
  /** Wake-on-LAN is enabled in settings */
  wolEnabled?: boolean
  /** Named WOL targets saved in settings */
  wolTargets?: WolTarget[]
}>(), {
  atxAvailable: false,
  wolEnabled: false,
  wolTargets: () => [],
})

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'powerShort'): void
  (e: 'powerLong'): void
  (e: 'reset'): void
  (e: 'wol', macAddress: string): void
}>()

const { t } = useI18n()

// With only one feature enabled the tab strip is noise, so it is hidden and the
// remaining panel is shown on its own.
const showTabs = computed(() => props.atxAvailable && props.wolEnabled)
const activeTab = ref(props.atxAvailable ? 'atx' : 'wol')
const tabTriggerClass = 'h-8 rounded-md border-0 bg-transparent text-center text-xs text-muted-foreground shadow-none hover:text-foreground data-[state=active]:border-0 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm'

const powerState = ref<'on' | 'off' | 'unknown'>('unknown')
const hddState = ref<'active' | 'inactive' | 'unknown'>('unknown')
let powerStateTimer: number | null = null
let actionTimer: number | null = null

const selectedMac = ref('')
const wolSending = ref(false)
const activeAction = ref<AtxAction | null>(null)

const actionBusy = computed(() => activeAction.value !== null)

const powerStateIconColor = computed(() => {
  switch (powerState.value) {
    case 'on': return 'text-success'
    case 'off': return 'text-muted-foreground'
    default: return 'text-warning'
  }
})

const powerStateTextColor = computed(() => {
  switch (powerState.value) {
    case 'on': return 'text-success'
    default: return ''
  }
})

const powerStateText = computed(() => {
  switch (powerState.value) {
    case 'on': return t('atx.stateOn')
    case 'off': return t('atx.stateOff')
    default: return t('atx.stateUnknown')
  }
})

const hddStateIconColor = computed(() => {
  switch (hddState.value) {
    case 'active': return 'text-success'
    case 'inactive': return 'text-muted-foreground'
    default: return 'text-warning'
  }
})

const hddStateTextColor = computed(() => {
  switch (hddState.value) {
    case 'active': return 'text-success'
    default: return ''
  }
})

const hddStateText = computed(() => {
  switch (hddState.value) {
    case 'active': return t('atx.hddActive')
    case 'inactive': return t('atx.hddInactive')
    default: return t('atx.stateUnknown')
  }
})

function handleAction(action: AtxAction) {
  if (actionBusy.value) return

  console.log('[AtxPopover] Running action:', action)
  activeAction.value = action

  if (action === 'short') emit('powerShort')
  else if (action === 'long') emit('powerLong')
  else emit('reset')

  if (actionTimer !== null) {
    window.clearTimeout(actionTimer)
  }
  actionTimer = window.setTimeout(() => {
    activeAction.value = null
    actionTimer = null
    refreshPowerState().catch(() => {})
  }, Math.max(actionDurations[action], minActionFeedbackMs))
}

function sendWol() {
  if (!selectedMac.value || wolSending.value) return
  wolSending.value = true

  emit('wol', selectedMac.value)

  setTimeout(() => {
    wolSending.value = false
  }, 1000)
}

async function refreshPowerState() {
  if (!props.atxAvailable) return
  try {
    const state = await atxApi.status()
    powerState.value = state.power_status
    hddState.value = state.hdd_status
  } catch {
    powerState.value = 'unknown'
    hddState.value = 'unknown'
  }
}

onMounted(() => {
  if (!props.atxAvailable) return
  refreshPowerState().catch(() => {})
  powerStateTimer = window.setInterval(() => {
    refreshPowerState().catch(() => {})
  }, 3000)
})

onUnmounted(() => {
  if (powerStateTimer !== null) {
    window.clearInterval(powerStateTimer)
    powerStateTimer = null
  }
  if (actionTimer !== null) {
    window.clearTimeout(actionTimer)
    actionTimer = null
  }
})

// Fall back to whichever panel is still enabled if settings change while open.
watch(
  () => [props.atxAvailable, props.wolEnabled] as const,
  ([atxAvailable, wolEnabled]) => {
    if (!atxAvailable && activeTab.value === 'atx' && wolEnabled) activeTab.value = 'wol'
    if (!wolEnabled && activeTab.value === 'wol' && atxAvailable) activeTab.value = 'atx'
  },
  { immediate: true },
)

// Keep a valid selection whenever the saved target list changes.
watch(
  () => props.wolTargets,
  (targets) => {
    if (!targets.some(target => target.mac === selectedMac.value)) {
      selectedMac.value = targets[0]?.mac ?? ''
    }
  },
  { immediate: true, deep: true },
)
</script>

<template>
  <div class="p-2.5 space-y-2.5">
    <Tabs v-model="activeTab">
      <TabsList
        v-if="showTabs"
        class="grid h-auto w-full grid-cols-2 gap-1 rounded-md border border-border bg-muted p-0.5"
      >
        <TabsTrigger
          value="atx"
          :class="tabTriggerClass"
        >
          <Power class="size-3 mr-1" />
          {{ t('atx.title') }}
        </TabsTrigger>
        <TabsTrigger
          value="wol"
          :class="tabTriggerClass"
        >
          <Wifi class="size-3 mr-1" />
          WOL
        </TabsTrigger>
      </TabsList>

      <!-- ATX Tab -->
      <TabsContent v-if="atxAvailable" value="atx" :class="showTabs ? 'mt-2.5 space-y-2.5' : 'mt-0 space-y-2.5'">
        <!-- Status -->
        <div class="grid grid-cols-2 gap-2">
          <div class="flex min-w-0 items-center gap-2 rounded-md border bg-muted/40 px-2 py-1.5">
            <Power :class="['size-4 shrink-0', powerStateIconColor]" />
            <div class="min-w-0">
              <p class="truncate text-[11px] leading-none text-muted-foreground">{{ t('atx.powerState') }}</p>
              <p :class="['mt-1 truncate text-xs font-medium leading-none', powerStateTextColor]">{{ powerStateText }}</p>
            </div>
          </div>
          <div class="flex min-w-0 items-center gap-2 rounded-md border bg-muted/40 px-2 py-1.5">
            <HardDrive :class="['size-4 shrink-0', hddStateIconColor]" />
            <div class="min-w-0">
              <p class="truncate text-[11px] leading-none text-muted-foreground">{{ t('atx.hddState') }}</p>
              <p :class="['mt-1 truncate text-xs font-medium leading-none', hddStateTextColor]">{{ hddStateText }}</p>
            </div>
          </div>
        </div>

        <Separator />

        <!-- Power Actions -->
        <div class="space-y-1">
          <Button
            variant="outline"
            size="sm"
            :disabled="actionBusy"
            :class="[
              'w-full justify-start gap-2 h-8 text-xs',
              activeAction === 'short' ? 'bg-muted text-muted-foreground' : '',
            ]"
            @click="handleAction('short')"
          >
            <Power class="size-3" />
            {{ t('atx.shortPress') }}
          </Button>

          <Button
            variant="outline"
            size="sm"
            :disabled="actionBusy"
            :class="[
              'h-8 w-full justify-start gap-2 text-xs text-warning hover:bg-warning/10 hover:text-warning',
              activeAction === 'long' ? 'bg-muted text-muted-foreground hover:text-muted-foreground hover:bg-muted dark:hover:bg-muted' : '',
            ]"
            @click="handleAction('long')"
          >
            <CircleDot class="size-3" />
            {{ t('atx.longPress') }}
          </Button>

          <Button
            variant="outline"
            size="sm"
            :disabled="actionBusy"
            :class="[
              'h-8 w-full justify-start gap-2 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive',
              activeAction === 'reset' ? 'bg-muted text-muted-foreground hover:text-muted-foreground hover:bg-muted dark:hover:bg-muted' : '',
            ]"
            @click="handleAction('reset')"
          >
            <RotateCcw class="size-3" />
            {{ t('atx.reset') }}
          </Button>
        </div>
      </TabsContent>

      <!-- WOL Tab -->
      <TabsContent v-if="wolEnabled" value="wol" :class="showTabs ? 'mt-2.5 space-y-2.5' : 'mt-0 space-y-2.5'">
        <p v-if="!showTabs" class="flex items-center gap-1.5 text-xs font-medium">
          <Wifi class="size-3.5" />
          {{ t('atx.wol') }}
        </p>

        <template v-if="wolTargets.length > 0">
          <div class="space-y-1">
            <Button
              v-for="target in wolTargets"
              :key="target.mac"
              variant="outline"
              size="sm"
              :class="[
                'h-auto w-full justify-start gap-2 px-2 py-1.5 text-left',
                target.mac === selectedMac ? 'border-primary bg-primary/5' : '',
              ]"
              @click="selectedMac = target.mac"
            >
              <Check :class="['size-3.5 shrink-0', target.mac === selectedMac ? 'opacity-100' : 'opacity-0']" />
              <span class="min-w-0 flex-1">
                <span class="block truncate text-xs font-medium leading-none">
                  {{ target.name || t('atx.wolUnnamedTarget') }}
                </span>
                <span class="mt-1 block truncate font-mono text-[11px] leading-none text-muted-foreground">
                  {{ target.mac }}
                </span>
              </span>
            </Button>
          </div>

          <Button
            size="sm"
            class="h-8 w-full gap-2 text-xs"
            :disabled="!selectedMac || wolSending"
            @click="sendWol"
          >
            <Send class="size-3.5" />
            {{ wolSending ? t('atx.wolSending') : t('atx.wolSend') }}
          </Button>
        </template>

        <p v-else class="rounded-md border border-dashed px-2 py-3 text-center text-xs text-muted-foreground">
          {{ t('atx.wolNoTargets') }}
        </p>
      </TabsContent>
    </Tabs>
  </div>
</template>
