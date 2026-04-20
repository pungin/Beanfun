/**
 * Game-code constants for the per-game Tools dialog stack
 * (P12.5 D7).
 *
 * # WPF parity
 *
 * Two distinct WPF call sites consume these codes, with two
 * distinct semantic concerns:
 *
 * 1. **Visibility gate** (`MainWindow.xaml.cs::selectedGameChanged`
 *    L621 / L630-633 → both `accountList.btn_Tools.Visibility`
 *    L1710-1713 and `settingPage.btn_Tools.Visibility`):
 *    "Should the Tools button be rendered at all for the active
 *    game?" → answered by {@link TOOLS_GAME_CODES}.
 *
 * 2. **Routing dispatch** (`AccountList.xaml.cs::btn_Tools_Click`
 *    L237-250 → `switch (gameCode) { case "610074_T9": case
 *    "610075_T9": new MapleTools().Show(); break; case "610096_TE":
 *    new KartTools().Show(); break; }`): "Which tools window
 *    should this click open?" → answered by {@link MAPLE_TOOLS_CODES}
 *    + {@link KART_TOOLS_CODE}.
 *
 * # Why visibility and routing live in separate constants
 *
 * `TOOLS_GAME_CODES` is the **union** (`MapleTools` codes ∪
 * `KartTools` code); `MAPLE_TOOLS_CODES` and `KART_TOOLS_CODE`
 * are the **partition**. Mathematically the visibility set is
 * derivable from the routing partition (`new Set([
 * ...MAPLE_TOOLS_CODES, KART_TOOLS_CODE])`), but materialising
 * it as a literal `TOOLS_GAME_CODES` constant has two payoffs:
 *
 * - The visibility gate is consulted on every game-switch
 *   (`MainWindow::selectedGameChanged` L621 fires every time).
 *   A literal `Set` is a single allocation at module load; a
 *   derived `new Set([...])` recomputes the membership table
 *   on every consultation. Tiny perf delta, but cleaner intent.
 * - Decoupling visibility from routing means a future "show
 *   the Tools button for a game whose tools window isn't ready
 *   yet" or "hide a tools window from the menu without removing
 *   the routing" change is a one-line edit to the right set.
 *   Mixing the two would force every such edit to touch the
 *   routing partition (a fragile coupling — e.g. accidentally
 *   removing a code from the partition would silently disable
 *   both visibility and routing in lock-step, hiding the bug
 *   intent from code review).
 *
 * The "single source of truth" cost is one comment per consumer
 * call site explaining which set it should consult; the cost of
 * conflating them would be a class of regressions where every
 * routing change risks accidentally hiding the button (or
 * vice-versa).
 *
 * # Why a `Set` (not `Array.includes`)
 *
 * O(1) membership check vs. O(n) for `[...].includes(...)`. The
 * size is small enough that the perf delta is irrelevant, but
 * the `Set` form better signals intent ("this is a membership
 * test, not an ordered collection") and matches the existing
 * `UNCONNECTED_GAME_CODES` convention in `stores/game.ts`.
 *
 * # Why these aren't loaded from backend metadata
 *
 * The codes are hard-coded in the WPF source (no Beanfun-server
 * config), so a backend lookup would be cargo-cult layering with
 * no payoff. Adding a new tools-bearing game is a one-line edit
 * here mirroring the WPF C# diff.
 */

/**
 * MapleStory game codes that route to `windows/MapleTools.vue`.
 *
 * Verbatim from `AccountList.xaml.cs` L242-243:
 *
 * - `610074_T9` — MapleStory TW
 * - `610075_T9` — MapleStory M (mobile bridge)
 *
 * Both share the identical MapleTools window in WPF (the switch
 * arms fall through to the same `new MapleTools().Show()` body),
 * so a single set captures the routing rule.
 */
export const MAPLE_TOOLS_CODES: ReadonlySet<string> = new Set(['610074_T9', '610075_T9'])

/**
 * KartRider game code that routes to `windows/KartTools.vue`.
 *
 * Verbatim from `AccountList.xaml.cs` L246: `case "610096_TE":
 * new KartTools().Show()`. A single string (not a `Set`) because
 * WPF only declares one KartRider entry — collapsing a 1-element
 * set into a literal saves the membership-test ceremony at the
 * call sites.
 */
export const KART_TOOLS_CODE = '610096_TE'

/**
 * Union of every game code with a Tools window. Drives the
 * `v-if="showToolsButton"` gate on both `AccountList.vue` and
 * `Settings.vue`, mirroring WPF `MainWindow.xaml.cs` L621
 * (Settings page) and L1710-1713 (AccountList page) — the same
 * gate predicate runs in both call sites in WPF, so we expose a
 * single set both pages consume.
 *
 * Materialised as a literal (not derived from
 * `MAPLE_TOOLS_CODES` ∪ `KART_TOOLS_CODE`) — see the module
 * docblock for the rationale. The contract is enforced by the
 * `tests/unit/constants/tools.spec.ts` invariants that pin
 * `TOOLS_GAME_CODES` to be the exact union of the routing
 * constants, so a divergence between them is caught at CI time
 * rather than at click time.
 */
export const TOOLS_GAME_CODES: ReadonlySet<string> = new Set([
  '610074_T9',
  '610075_T9',
  '610096_TE',
])
