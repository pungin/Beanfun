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
 */
export const CLASSIC_ELIGIBLE_GAME_CODES: ReadonlySet<string> = new Set(['610074_T9'])

/**
 * Tauri events emitted by the backend classic launcher
 * (`commands::classic`) — keep in sync with the Rust constants.
 */
export const CLASSIC_LAUNCHED_EVENT = 'classic-launched'
export const CLASSIC_FAILED_EVENT = 'classic-launch-failed'
export const CLASSIC_TIMEOUT_EVENT = 'classic-launch-timeout'

/** Official Nexon Game Manager installer (shown when the self-check
 * finds no `ngm://` handler). */
export const NGM_DOWNLOAD_URL = 'https://platform.nexon.com/NGM/Bin/Install_NGM.exe'
