/**
 * Config store — typed K-V wrapper around backend `Config.xml`.
 *
 * The backend owns `Config.xml` (load / save / mutate); this store
 * is an in-memory cache the frontend boots by calling
 * {@link useConfigStore.loadAll} once. Subsequent `get(key)` calls
 * read from the cache without an IPC round-trip; `set(key, value)`
 * writes through to the backend and updates the cache atomically.
 *
 * # Why no persistence (P11 Q5 = B)
 *
 * `Config.xml` is the single source of truth. Mirroring it into
 * `localStorage` would create a two-way sync problem (which copy
 * wins on reload?) for zero observable benefit — boot reload is
 * sub-100ms even with a couple hundred config keys.
 *
 * # API surface mirrors backend signature
 *
 * - `loadAll()` ↔ `commands.getAllConfig()`
 * - `get(key)`  ↔ local cache lookup (no IPC)
 * - `set(key, null)` ↔ `commands.setConfig(key, null)` (delete semantics)
 *
 * The store does **not** expose `commands.getConfigValue` because the
 * single-key fetch path is only useful before `loadAll()` runs, and
 * we always boot with `loadAll()`. Callers that genuinely need the
 * single-key fallback can `wrapCommand(commands.getConfigValue(key))`
 * directly — the store wouldn't add value there.
 */

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { commands } from '../types/bindings'
import { safeInvoke, wrapCommand } from '../services/invoke'

export const useConfigStore = defineStore('config', () => {
  /**
   * Reactive K-V cache. Reset to `{}` until {@link loadAll} succeeds;
   * deleting a key ({@link set} with `value === null`) removes the
   * entry rather than storing a sentinel so downstream consumers can
   * use `key in entries.value` checks.
   */
  const entries = ref<Record<string, string>>({})

  /**
   * `true` once {@link loadAll} has resolved at least once. Lets the
   * UI distinguish "still booting" from "loaded but key missing".
   */
  const loaded = ref(false)

  /** Number of cached keys — handy for diagnostics & smoke tests. */
  const size = computed(() => Object.keys(entries.value).length)

  /**
   * Load the full Config.xml snapshot into the cache. Call once at
   * boot; subsequent calls overwrite the cache (useful after import
   * / restore flows in P12).
   *
   * Errors propagate via {@link wrapCommand} (toast + throw) so the
   * boot sequence in `App.vue` can decide whether to fall back to
   * defaults or block startup.
   */
  async function loadAll(): Promise<void> {
    const all = await wrapCommand(commands.getAllConfig())
    const next: Record<string, string> = {}
    for (const [key, value] of Object.entries(all)) {
      if (typeof value === 'string') next[key] = value
    }
    entries.value = next
    loaded.value = true
  }

  /**
   * Read a key from the in-memory cache. Returns `undefined` for
   * missing keys (callers should provide defaults). No IPC.
   */
  function get(key: string): string | undefined {
    return entries.value[key]
  }

  /**
   * Read a key with a typed fallback. Convenience wrapper around
   * {@link get} so callers don't repeat the `?? defaultValue`
   * pattern at every call site.
   */
  function getOr(key: string, fallback: string): string {
    return entries.value[key] ?? fallback
  }

  /**
   * Persist a key to backend Config.xml and update the local cache.
   *
   * If the file is read-only (user locked their settings on purpose),
   * the backend write fails silently and only the in-memory cache is
   * updated — matching WPF's silent `catch{}` at
   * `ConfigAppSettings.cs` L60. The current session keeps working;
   * the on-disk file stays untouched so the locked values survive
   * across restarts.
   *
   * @param key — config key (matches WPF naming verbatim)
   * @param value — string to set, or `null` to delete the key
   */
  async function set(key: string, value: string | null): Promise<void> {
    const result = await safeInvoke(commands.setConfig(key, value))
    if (!result.ok) {
      console.warn(`[config] set "${key}" failed (file may be read-only), using in-memory value`, result.error)
    }
    if (value === null) {
      delete entries.value[key]
    } else {
      entries.value[key] = value
    }
  }

  return {
    entries,
    loaded,
    size,
    loadAll,
    get,
    getOr,
    set,
  }
})
