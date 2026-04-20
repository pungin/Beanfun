/**
 * P12.5 D7 — invariants for `src/constants/tools.ts`.
 *
 * The Tools dialog stack relies on three constants:
 *
 * - `TOOLS_GAME_CODES` — visibility gate for both
 *   `pages/AccountList.vue` and `pages/Settings.vue`.
 * - `MAPLE_TOOLS_CODES` — routing partition for MapleTools.
 * - `KART_TOOLS_CODE`   — routing partition for KartTools.
 *
 * The constants module docblock argues that the visibility set
 * should be materialised independently from the routing
 * partition (so a future "show button without routing yet"
 * change is a one-line edit). That decoupling is only safe if
 * we lock the *current* invariant — visibility set ≡ routing
 * union — at CI time, otherwise the two sets can drift
 * silently and produce a class of bugs where the button is
 * visible but does nothing (or invisible but routable).
 *
 * These tests pin the contract WPF currently has
 * (`AccountList.xaml.cs::btn_Tools_Click` L237-250 +
 * `MainWindow.xaml.cs::selectedGameChanged` L621 / L630-633 /
 * L1710-1713) so any future drift breaks the build.
 */

import { describe, expect, it } from 'vitest'
import { KART_TOOLS_CODE, MAPLE_TOOLS_CODES, TOOLS_GAME_CODES } from '../../../src/constants/tools'

describe('constants/tools', () => {
  describe('MAPLE_TOOLS_CODES', () => {
    it('exactly matches the two WPF-whitelisted MapleStory codes', () => {
      /*
       * WPF `AccountList.xaml.cs` L242-243:
       *   case "610074_T9":   // MapleStory TW
       *   case "610075_T9":   // MapleStory M
       *     new MapleTools().Show();
       *
       * Adding / removing a code here means the routing partition
       * and the WPF source have drifted; either WPF was updated
       * upstream and this constant needs to follow, or the SPA
       * is silently routing differently from the desktop client.
       */
      expect([...MAPLE_TOOLS_CODES].sort()).toEqual(['610074_T9', '610075_T9'])
    })

    it('does not include the KartRider code', () => {
      /*
       * Routing partition invariant: a code can only belong to
       * one of MAPLE / KART, never both. `ToolsDialogStack`
       * checks the KartRider code first to be defensive against
       * a future typo, but the partition itself must hold so the
       * "first match wins" ordering doesn't accidentally start
       * mattering.
       */
      expect(MAPLE_TOOLS_CODES.has(KART_TOOLS_CODE)).toBe(false)
    })
  })

  describe('KART_TOOLS_CODE', () => {
    it('matches the WPF KartRider code verbatim', () => {
      /*
       * WPF `AccountList.xaml.cs` L246: case "610096_TE":
       *   new KartTools().Show();
       */
      expect(KART_TOOLS_CODE).toBe('610096_TE')
    })
  })

  describe('TOOLS_GAME_CODES', () => {
    it('is exactly the union of MAPLE_TOOLS_CODES ∪ {KART_TOOLS_CODE}', () => {
      /*
       * Visibility-equals-routing-union invariant. The constants
       * docblock explains why the two sets are materialised
       * separately rather than derived; this test makes the
       * derivation a one-step "compute and compare" so future
       * edits to either side surface as a CI failure rather
       * than as a runtime ghost button.
       */
      const expected = new Set<string>([...MAPLE_TOOLS_CODES, KART_TOOLS_CODE])
      expect([...TOOLS_GAME_CODES].sort()).toEqual([...expected].sort())
    })

    it('matches the three WPF-whitelisted codes verbatim', () => {
      /*
       * Anchor the absolute values too — derived equality alone
       * would still pass if both sides were edited to a wrong
       * shared list.
       */
      expect([...TOOLS_GAME_CODES].sort()).toEqual(['610074_T9', '610075_T9', '610096_TE'])
    })

    it('rejects a known non-tools code (KartRider Rush+ TW 610099_T9)', () => {
      /*
       * Negative case — `610099_T9` (KartRider Rush+ TW) is a
       * connected game with no Tools window in WPF. The visibility
       * gate must keep the button hidden.
       */
      expect(TOOLS_GAME_CODES.has('610099_T9')).toBe(false)
    })

    it('rejects an unconnected game code (Mabinogi 610153_TN)', () => {
      /*
       * Defensive — unconnected games (`UNCONNECTED_GAME_CODES`
       * in `stores/game.ts`) never overlap the tools whitelist.
       * If WPF ever ships an unconnected-game tools window this
       * test should be the trip wire.
       */
      expect(TOOLS_GAME_CODES.has('610153_TN')).toBe(false)
    })
  })
})
