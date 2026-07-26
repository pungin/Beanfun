/**
 * MapleStory Classic (新楓之谷經典版 / 懷舊服) frontend constants.
 *
 * Classic is launched through the Gamania galaxy SSO + Nexon Game
 * Manager — see `src-tauri/src/commands/classic.rs` for the whole flow.
 * The frontend's job is only to gate + trigger it.
 */

/**
 * Game codes whose game bar offers the Classic launch button.
 * Classic is a MapleStory offshoot, so it rides along with the regular
 * MapleStory selection (TW/HK share `610074_T9`).
 *
 * The button is additionally gated on region: only **HK** shares one
 * beanfun login between the regular service and Classic. A TW classic
 * login is entirely separate, so offering "switch to Classic" from a
 * signed-in TW account list would just pop another login form — TW
 * users start Classic from the login page's 懷舊服 mode instead.
 */
export const CLASSIC_ELIGIBLE_GAME_CODES: ReadonlySet<string> = new Set(['610074_T9'])

/**
 * Tauri events emitted by the backend classic launcher
 * (`commands::classic`) — keep in sync with the Rust constants.
 */
export const CLASSIC_LAUNCHED_EVENT = 'classic-launched'
export const CLASSIC_FAILED_EVENT = 'classic-launch-failed'
/**
 * Emitted past the soft deadline. NOT a failure — an observed launch
 * took 37s and one measured run landed 7s after an earlier build had
 * already cried failure, so the backend keeps watching and may still
 * emit {@link CLASSIC_LAUNCHED_EVENT} afterwards.
 */
export const CLASSIC_SLOW_EVENT = 'classic-launch-slow'
/** Emitted when the portal needs an interactive sign-in (always TW). */
export const CLASSIC_NEEDS_LOGIN_EVENT = 'classic-needs-login'

/** Official Nexon Game Manager installer (shown when the self-check
 * finds no `ngm://` handler). */
export const NGM_DOWNLOAD_URL = 'https://platform.nexon.com/NGM/Bin/Install_NGM.exe'
