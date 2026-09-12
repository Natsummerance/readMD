// ReadMD companion layer shared by both overlay renderers.  It renders the
// speech-bubble line system and the idle companionship state machine that the
// in-reader pet has, then maps moods onto the Live2D stage (talking mouth,
// drowsy eyelids, closed eyes) when that renderer is active.  All user-facing
// wording arrives through the host bridge (`info.lines`, localized by the
// Python host from the shared i18n files), so this module contains no copy.

import type { Live2dLifeController } from './live2d/stage'

type PetOverlayApi = {
  control: (command: Record<string, unknown>) => void
  onState: (listener: (state: unknown) => void) => void
}

type LifeState = {
  info?: { lines?: Record<string, string>; locale?: string; petName?: string }
  activity?: { busy?: boolean; error?: boolean; justCompleted?: boolean }
}

type Priority = 1 | 2 | 3

const CHATTER = 1 as Priority
const EVENT = 2 as Priority
const POKE = 3 as Priority

// Companionship thresholds, mirroring the in-reader pet's cadence.
const BORED_AFTER_MS = 90_000
const DOZING_AFTER_MS = 4 * 60_000
const SLEEPING_AFTER_MS = 10 * 60_000
const CHATTER_EVERY_MS = 55_000
const CHATTER_JITTER_MS = 25_000
const POKE_COMBO_COUNT = 3
const POKE_COMBO_WINDOW_MS = 1500
const EVENT_THROTTLE_MS = 30_000

function petOverlayApi(): PetOverlayApi | undefined {
  return (window as unknown as { hermesDesktop?: { petOverlay: PetOverlayApi } }).hermesDesktop?.petOverlay
}

const now = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now())

function pick<T>(items: T[]): T | undefined {
  return items.length ? items[Math.floor(Math.random() * items.length)] : undefined
}

function injectStyles(): void {
  if (document.getElementById('readmd-pet-life-style')) return
  const style = document.createElement('style')
  style.id = 'readmd-pet-life-style'
  style.textContent = `
.readmd-pet-life {
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 2147483647;
  font-family: "Segoe UI", "Microsoft YaHei", system-ui, sans-serif;
}
.readmd-pet-life__bubble {
  position: absolute;
  left: 50%;
  bottom: 160px;
  top: auto;
  transform: translateX(-50%) translateY(6px) scale(0.96);
  max-width: 82%;
  padding: 8px 12px;
  border-radius: 12px;
  background: rgba(28, 30, 38, 0.92);
  color: #f5f6fa;
  font-size: 13px;
  line-height: 1.45;
  text-align: center;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
  opacity: 0;
  transition: opacity 180ms ease, transform 180ms ease;
}
.readmd-pet-life__bubble.is-visible {
  opacity: 1;
  transform: translateX(-50%) translateY(0) scale(1);
}
.readmd-pet-life__bubble::after {
  content: "";
  position: absolute;
  left: 50%;
  bottom: -6px;
  width: 12px;
  height: 12px;
  transform: translateX(-50%) rotate(45deg);
  background: rgba(28, 30, 38, 0.92);
  border-radius: 2px;
}
.readmd-pet-life__veil {
  position: absolute;
  inset: 0;
  opacity: 0;
  transition: opacity 1200ms ease;
  background: radial-gradient(ellipse 70% 55% at 50% 68%, rgba(10, 12, 24, 0.55), rgba(10, 12, 24, 0) 70%);
}
.readmd-pet-life.is-sleeping .readmd-pet-life__veil {
  opacity: 1;
}
`
  document.head.appendChild(style)
}

export type PetLifeOptions = { live2d?: Live2dLifeController }

export function mountPetLife(options: PetLifeOptions = {}): void {
  const api = petOverlayApi()
  const probeActive = new URLSearchParams(window.location.search).has('live2dProbe')
  injectStyles()

  const layer = document.createElement('div')
  layer.className = 'readmd-pet-life'
  const veil = document.createElement('div')
  veil.className = 'readmd-pet-life__veil'
  const bubble = document.createElement('div')
  bubble.className = 'readmd-pet-life__bubble'
  layer.appendChild(veil)
  layer.appendChild(bubble)
  document.body.appendChild(layer)

  function getCharacterTop(): number {
    if (typeof options.live2d?.getCharacterTop === 'function') {
      const top = options.live2d.getCharacterTop()
      if (Number.isFinite(top) && top > 0) return top
    }
    const canvas = document.querySelector('canvas')
    if (canvas) {
      const rect = canvas.getBoundingClientRect()
      if (rect.height > 0 && rect.top > 0) return rect.top
    }
    return window.innerHeight * 0.55
  }

  function updateBubblePosition(): void {
    const charTop = getCharacterTop()
    const fromBottom = window.innerHeight - charTop
    let targetBottom = fromBottom + 10

    const bubbleH = bubble.offsetHeight || 44
    const maxBottom = window.innerHeight - bubbleH - 12
    if (targetBottom > maxBottom) {
      targetBottom = Math.max(12, maxBottom)
    }

    bubble.style.top = 'auto'
    bubble.style.bottom = `${Math.round(targetBottom)}px`
  }

  window.addEventListener('resize', updateBubblePosition)

  let lines: Record<string, string> = {}
  let shown: { text: string; priority: Priority; until: number } | undefined
  let hideTimer: number | undefined
  let lastInteraction = now()
  let lastChatter = now()
  let chatterDelay = CHATTER_EVERY_MS
  let phase: 'awake' | 'bored' | 'dozing' | 'sleeping' = 'awake'
  let phaseAnnounced = ''
  let pokeTimes: number[] = []
  let pokeIndex = 0
  let idleIndex = 0
  let lastEventAt: Record<string, number> = {}
  let sawActivity: { busy?: boolean; error?: boolean; justCompleted?: boolean } = {}
  let greeted = false

  const line = (key: string): string => (lines[key] || '').trim()

  function hideBubble(): void {
    if (hideTimer !== undefined) {
      window.clearTimeout(hideTimer)
      hideTimer = undefined
    }
    shown = undefined
    bubble.classList.remove('is-visible')
    options.live2d?.setTalking(false)
  }

  function show(text: string, priority: Priority): void {
    if (!text) return
    const moment = now()
    if (shown && moment < shown.until && priority <= shown.priority) return
    shown = { text, priority, until: moment + 10_000 }
    bubble.textContent = text
    updateBubblePosition()
    bubble.classList.add('is-visible')
    options.live2d?.setTalking(true)
    if (hideTimer !== undefined) window.clearTimeout(hideTimer)
    // Reading-speed estimate: keep each line on screen long enough to finish.
    const duration = Math.min(7000, 2400 + text.length * 110)
    hideTimer = window.setTimeout(() => { hideBubble() }, duration)
    requestAnimationFrame(() => {
      updateBubblePosition()
    })
  }

  function interacted(): void {
    const moment = now()
    pokeTimes = pokeTimes.filter(time => moment - time <= POKE_COMBO_WINDOW_MS)
    pokeTimes.push(moment)
    if (pokeTimes.length >= POKE_COMBO_COUNT) {
      pokeTimes = []
      const combo = [line('pet.pokeQuote2'), line('pet.pokeQuote3'), line('pet.pokeQuote4')].filter(Boolean)
      if (combo.length) show(pick(combo)!, POKE)
    } else {
      const single = line('pet.pokeQuote1')
      if (single && pokeTimes.length === 1) show(single, CHATTER)
    }
    if (phase !== 'awake') {
      phase = 'awake'
      phaseAnnounced = ''
      options.live2d?.setMood('normal')
      layer.classList.remove('is-sleeping')
      const back = line('pet.returnQuote')
      if (back) show(back, EVENT)
    }
    lastInteraction = moment
  }

  function handleActivity(activity: LifeState['activity']): void {
    const busy = Boolean(activity?.busy)
    const error = Boolean(activity?.error)
    const completed = Boolean(activity?.justCompleted)
    const moment = now()
    const edge = (kind: string, previous: boolean, current: boolean): boolean => {
      if (!current || previous) return false
      const previousAt = lastEventAt[kind]
      // A kind that never fired must not be throttled by the epoch.
      return previousAt === undefined || moment - previousAt > EVENT_THROTTLE_MS
    }
    if (edge('busy', Boolean(sawActivity.busy), busy)) {
      lastEventAt.busy = moment
      show(line('pet.taskBusy') || line('pet.bubbleDropReceived'), EVENT)
    }
    if (edge('error', Boolean(sawActivity.error), error)) {
      lastEventAt.error = moment
      show(line('pet.taskError'), EVENT)
    }
    if (edge('done', Boolean(sawActivity.justCompleted), completed)) {
      lastEventAt.done = moment
      show(line('pet.reading100'), EVENT)
      options.live2d?.celebrate()
    }
    sawActivity = { busy, error, justCompleted: completed }
  }

  function tick(): void {
    const moment = now()
    if (shown && moment >= shown.until) hideBubble()
    if (phase === 'sleeping') return

    if (moment - lastInteraction >= SLEEPING_AFTER_MS && phase === 'dozing') {
      phase = 'sleeping'
      const text = line('pet.sleepQuote1')
      if (text && text !== phaseAnnounced) show(text, EVENT)
      phaseAnnounced = text
      options.live2d?.setMood('sleeping')
      layer.classList.add('is-sleeping')
      options.live2d?.setTalking(false)
      return
    }
    if (moment - lastInteraction >= DOZING_AFTER_MS && (phase === 'awake' || phase === 'bored')) {
      phase = 'dozing'
      const text = line('pet.sleepQuote2')
      if (text && text !== phaseAnnounced) show(text, EVENT)
      phaseAnnounced = text
      options.live2d?.setMood('drowsy')
      return
    }
    if (moment - lastInteraction >= BORED_AFTER_MS && phase === 'awake') {
      phase = 'bored'
    }
    if (phase === 'bored' && moment - lastChatter >= chatterDelay) {
      const bored = [line('pet.bubbleQuote4'), line('pet.idleQuote1'), line('pet.idleQuote2'), line('pet.idleQuote3')].filter(Boolean)
      if (bored.length) show(pick(bored)!, CHATTER)
      lastChatter = moment
      chatterDelay = CHATTER_EVERY_MS + Math.random() * CHATTER_JITTER_MS
    }
  }

  function greet(state: LifeState): void {
    if (greeted) return
    greeted = true
    lines = state.info?.lines && typeof state.info.lines === 'object' ? state.info.lines : {}
    const hour = new Date().getHours()
    const greetingKey =
      hour >= 5 && hour < 8 ? 'pet.greetingEarlyMorning'
      : hour >= 8 && hour < 12 ? 'pet.greetingMorning'
      : hour >= 12 && hour < 14 ? 'pet.greetingNoon'
      : hour >= 14 && hour < 18 ? 'pet.greetingAfternoon'
      : hour >= 18 && hour < 22 ? 'pet.greetingEvening'
      : 'pet.greetingNight'
    show(line(greetingKey) || line('pet.bubbleQuote1'), CHATTER)
    lastChatter = now()
  }

  api?.onState(next => {
    const state = (next || {}) as LifeState
    if (probeActive) console.log('[pet-life] state push', JSON.stringify({ activity: state.activity, hasLines: Boolean(state.info?.lines) }))
    if (state.info?.lines) lines = state.info.lines
    handleActivity(state.activity)
    greet(state)
  })
  // Ask the host to (re)publish current state now that this listener is mounted.
  api?.control({ type: 'ready' })

  if (probeActive) {
    (window as unknown as { __petLifeDebug?: object }).__petLifeDebug = {
      get phase(): string { return phase },
      get lines(): Record<string, string> { return lines },
      get shown(): { text: string } | undefined { return shown }
    }
  }

  window.addEventListener('pointerdown', event => {
    if (event.button !== 0) return
    interacted()
  })
  window.addEventListener('pointermove', event => {
    if (event.buttons === 0) return
    lastInteraction = now()
  })

  window.setInterval(tick, 1000)
}

export { mountPetLife as default }
