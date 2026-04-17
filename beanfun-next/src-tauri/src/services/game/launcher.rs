//! Game-launch primitives + Normal-mode dispatch.
//!
//! Pure helpers that cover chunk 8.1's slice of WPF's
//! `btn_Run_Game_Click` (`Beanfun/MainWindow.xaml.cs` L1727-1900):
//!
//! | Helper                          | WPF origin                                                |
//! | ------------------------------- | --------------------------------------------------------- |
//! | [`validate_path`]               | L1748 (empty / missing) + L1753-1762 (non-ASCII)          |
//! | [`GameStartMode`]               | L32-37 `enum GameStartMode`                               |
//! | [`GameStartMode::try_from`]     | L1837 `int.Parse` + L1863-1864 `> LR → clamp`             |
//! | [`locale_to_resolved_mode`]     | L1840-1860 `switch (GetSystemDefaultLocaleName())`        |
//! | [`resolve_mode`]                | L1838-1864 overall Auto-resolution                        |
//! | [`substitute_credentials`]      | L1866-1879 `%s` double-replace                            |
//! | [`launch_normal`]               | L1886-1891 `Process.Start(startInfo)` with WorkingDirectory |
//!
//! The LocaleRemulator branch (L1883-1885 → [`startByLR`][src-wpf])
//! and the top-level `launch_game` orchestrator arrive in chunk 8.2 —
//! this file intentionally stops short of dispatching by mode.
//!
//! [src-wpf]: https://github.com/pungin/Beanfun/blob/main/Beanfun/MainWindow.xaml.cs#L1902
//!
//! # WPF behaviour departures (deliberate, documented)
//!
//! - **XP check dropped**: WPF L1850-1853 `App.OSVersion < WinVista`
//!   was dead code on every target beanfun-next supports (Tauri 2
//!   minimum Windows 7 SP1). Removed with a doc pointer at
//!   [`resolve_mode`].
//! - **Non-ASCII check uses Unicode scalar `> 128`** rather than UTF-16
//!   code unit `> 128`. For paths with no surrogate-pair characters
//!   (the realistic case — game-installation paths under Program Files)
//!   both tests give the same answer; the scalar version is simply
//!   what Rust makes natural via [`str::chars`].
//! - **`LocaleRemulatorUnsupported` variant not declared**: if the
//!   Win32 locale query fails we fall back to
//!   [`ResolvedMode::LocaleRemulator`] (the pessimistic default WPF
//!   uses for non-zh locales at L1857). No failure surface needed.
//! - **Windows uses `raw_arg` for the command line**: WPF passes
//!   `ProcessStartInfo.Arguments` verbatim to `CreateProcess` — Rust's
//!   default [`std::process::Command::arg`] adds quoting when whitespace
//!   is present, which breaks games whose CRT argv parser expects raw
//!   space-separated tokens. See [`launch_normal`] for the detailed
//!   rationale.
//!
//! # Cross-platform stance
//!
//! Every helper in this file compiles and runs on non-Windows so
//! `cargo test` works on macOS / Linux during development. The
//! only Win32-touching piece, `query_system_locale`, has a
//! non-Windows stub that returns `None` — resolution then falls
//! back to [`ResolvedMode::LocaleRemulator`], matching the "unknown
//! locale → LR" branch of WPF's switch.

use std::path::Path;

use super::error::GameError;

// ---------------------------------------------------------------------------
// Mode enums
// ---------------------------------------------------------------------------

/// User-selected launch mode, mirroring WPF `enum GameStartMode`
/// (`Beanfun/MainWindow.xaml.cs` L32-37).
///
/// The integer repr matches WPF so a config file saved by the
/// legacy launcher deserialises cleanly into the new enum via
/// [`GameStartMode::try_from`].
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameStartMode {
    /// Decide `Normal` vs `LocaleRemulator` based on the current
    /// system default locale — the default config value.
    Auto = 0,
    /// Directly `Process.Start` the game binary with its
    /// working directory set to the containing folder. Used on
    /// Traditional-Chinese locales where the game runs fine in
    /// native codepage.
    Normal = 1,
    /// Launch via `LRProc.exe` (bundled LocaleRemulator) so the
    /// game sees a Traditional-Chinese locale regardless of the
    /// system default. Used on non-TC systems where the game's
    /// ANSI/CP950 code path blows up under the native locale.
    LocaleRemulator = 2,
}

impl TryFrom<i32> for GameStartMode {
    type Error = i32;

    /// Parse an integer from the legacy config file.
    ///
    /// WPF L1863-1864 clamps values `> LocaleRemulator` down to
    /// `LocaleRemulator`, treating every "unknown positive" as a
    /// request for the safer path. We mirror the clamp; negative
    /// values (never produced by WPF but conceivable via hand-edited
    /// config) are rejected with `Err(value)` so the caller can
    /// decide whether to fall back to `Auto`.
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Auto),
            1 => Ok(Self::Normal),
            v if v >= 2 => Ok(Self::LocaleRemulator),
            v => Err(v),
        }
    }
}

/// Outcome of resolving [`GameStartMode::Auto`] against the system
/// locale — only the two concrete modes the rest of the pipeline can
/// dispatch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedMode {
    Normal,
    LocaleRemulator,
}

// ---------------------------------------------------------------------------
// Path validation
// ---------------------------------------------------------------------------

/// Verify `path` is non-empty, exists, and contains no non-ASCII
/// characters.
///
/// Implements the three preflight guards WPF runs before the
/// process / kill / launch phase (`MainWindow.xaml.cs` L1748 +
/// L1753-1762). Returning the offending character + position means
/// the UI layer can produce a helpful error message like
/// "position 12: '遊' is not ASCII — rename the folder or move the
/// game to an ASCII-only path."
pub fn validate_path(path: &Path) -> Result<(), GameError> {
    let as_str = match path.to_str() {
        // `Path` can technically hold non-UTF8 bytes on Unix, but
        // Windows paths round-trip UTF-8 cleanly and the game binary
        // is Windows-only. An empty path also lands here.
        Some(s) => s,
        None => {
            return Err(GameError::PathNonAscii {
                path: path.to_path_buf(),
                offending_char: '\u{FFFD}',
                position: 0,
            });
        }
    };

    if as_str.is_empty() {
        return Err(GameError::PathEmpty);
    }

    if !path.exists() {
        return Err(GameError::PathNotFound {
            path: path.to_path_buf(),
        });
    }

    if let Some((position, offending_char)) =
        as_str.chars().enumerate().find(|(_, c)| (*c as u32) > 128)
    {
        return Err(GameError::PathNonAscii {
            path: path.to_path_buf(),
            offending_char,
            position,
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Mode resolution
// ---------------------------------------------------------------------------

/// Map a BCP-47 locale name (the kind `GetSystemDefaultLocaleName`
/// returns) to the [`ResolvedMode`] a zh-TC user would want.
///
/// Mirrors the switch in `MainWindow.xaml.cs` L1842-1848: any of
/// `zh-Hant / zh-CHT / zh-TW / zh-HK / zh-MO` keeps the game in
/// Normal mode; every other locale — including `zh-CN`, `en-US`,
/// `ja-JP`, `ko-KR` — routes through LocaleRemulator so the game
/// sees a Traditional-Chinese codepage.
///
/// Pulled out as a pure helper so mode resolution is unit-testable
/// without a Win32 call.
pub fn locale_to_resolved_mode(locale: &str) -> ResolvedMode {
    match locale {
        // WPF case arms verbatim.
        "zh-Hant" | "zh-CHT" | "zh-TW" | "zh-HK" | "zh-MO" => ResolvedMode::Normal,
        _ => ResolvedMode::LocaleRemulator,
    }
}

/// Resolve a [`GameStartMode`] (possibly [`Auto`][GameStartMode::Auto])
/// down to a concrete [`ResolvedMode`] the dispatcher can switch on.
///
/// On Windows, `Auto` calls `GetSystemDefaultLocaleName` and feeds
/// the result into [`locale_to_resolved_mode`]. If the Win32 call
/// fails (returns 0 or junk) we fall back to
/// [`ResolvedMode::LocaleRemulator`], matching the `default` arm of
/// WPF's switch — LR is the safer fallback for "unknown locale".
///
/// On non-Windows (development / CI), `Auto` always resolves to
/// [`ResolvedMode::LocaleRemulator`]; the service itself won't
/// dispatch LR outside Windows (that's gated at chunk 8.2) but
/// having resolution compile everywhere keeps the unit-test shape
/// cross-platform.
pub fn resolve_mode(mode: GameStartMode) -> ResolvedMode {
    match mode {
        GameStartMode::Normal => ResolvedMode::Normal,
        GameStartMode::LocaleRemulator => ResolvedMode::LocaleRemulator,
        GameStartMode::Auto => match query_system_locale() {
            Some(locale) => locale_to_resolved_mode(&locale),
            None => ResolvedMode::LocaleRemulator,
        },
    }
}

/// Read the system default locale (e.g. `"zh-TW"`, `"en-US"`).
///
/// Returns `None` when the Win32 call fails or when compiled for
/// a non-Windows target. Private because only [`resolve_mode`]
/// should drive mode decisions — other call sites that need the
/// locale string directly can be added if a real need arises.
#[cfg(windows)]
fn query_system_locale() -> Option<String> {
    use windows::Win32::Globalization::GetSystemDefaultLocaleName;

    // `LOCALE_NAME_MAX_LENGTH` from winnls.h is `85` (wide chars,
    // including the trailing NUL). The constant is not re-exported
    // by the `Win32_Globalization` feature of the `windows` crate
    // at version 0.58, so we inline the value with a source
    // reference rather than take on another feature flag just to
    // pull a single `const` in.
    const LOCALE_NAME_MAX_LENGTH: usize = 85;

    let mut buf = [0u16; LOCALE_NAME_MAX_LENGTH];
    let len = unsafe { GetSystemDefaultLocaleName(&mut buf) };
    if len <= 0 {
        return None;
    }
    // Win32 returns length *including* the trailing NUL — strip it
    // before decoding. `len` is guaranteed to be ≤ buf size by the
    // API contract.
    let trimmed = &buf[..(len - 1) as usize];
    String::from_utf16(trimmed).ok()
}

#[cfg(not(windows))]
fn query_system_locale() -> Option<String> {
    // Non-Windows development stub: no system-wide "Windows locale"
    // exists, so fall through to LR in [`resolve_mode`].
    None
}

// ---------------------------------------------------------------------------
// Command-line credential substitution
// ---------------------------------------------------------------------------

/// Inject `account` and `password` into the first two `%s`
/// placeholders of `template`, leaving any further `%s` untouched.
///
/// Mirrors the `Regex.Replace(commandLine, account, 1)` +
/// second-pass replace in WPF L1876-1879 byte-for-byte: the first
/// `%s` becomes `account`, the second becomes `password`, and any
/// third-or-later `%s` stays literal (a WPF-side quirk we preserve
/// for parity).
///
/// Pure: no escape / quoting is applied — the caller is responsible
/// for whatever shell-safety the surrounding process expects (the
/// game binary receives this verbatim as a CLI arg). Empty strings
/// for any parameter are fine — WPF relies on the caller guarding
/// with `account != ""` before invoking the substitution path, and
/// we do the same.
pub fn substitute_credentials(template: &str, account: &str, password: &str) -> String {
    template
        .replacen("%s", account, 1)
        .replacen("%s", password, 1)
}

// ---------------------------------------------------------------------------
// Normal-mode spawn
// ---------------------------------------------------------------------------

/// Launch the game binary directly (no LocaleRemulator wrapper).
///
/// Mirrors WPF L1886-1891:
///
/// ```csharp
/// ProcessStartInfo startInfo = new ProcessStartInfo(gamePath);
/// startInfo.WorkingDirectory = Path.GetDirectoryName(gamePath);
/// startInfo.Arguments = commandLine;
/// Process.Start(startInfo);
/// ```
///
/// # Argument passing (WPF-parity detail)
///
/// WPF's `ProcessStartInfo.Arguments` is appended **verbatim** to the
/// program name to form `CreateProcess`'s `lpCommandLine` — no quoting
/// or escaping. A template like `"/hb /u:user1 /p:pw1"` reaches the
/// game as three space-separated argv entries exactly as written.
///
/// Rust's [`std::process::Command::arg`] on Windows runs every
/// argument through `Command`'s escape routine, wrapping strings
/// that contain whitespace in double quotes before handing them to
/// `CreateProcess`. Feeding it a pre-joined `"/hb /u:user1 /p:pw1"`
/// yields `game.exe "/hb /u:user1 /p:pw1"`, which the game's CRT
/// argv parser sees as a **single** token — `%s`-substituted login
/// credentials therefore never reach the game.
///
/// We use [`std::os::windows::process::CommandExt::raw_arg`] on
/// Windows so the pre-joined template reaches `CreateProcess`
/// byte-for-byte, matching WPF. Non-Windows builds keep the
/// quoting-aware [`std::process::Command::arg`] — this module has
/// no production callers outside Windows and compile coverage is
/// the only goal there.
///
/// # Process lifetime
///
/// The spawned [`std::process::Child`] is dropped immediately —
/// beanfun-next is a fire-and-forget launcher; we never talk to the
/// game process after spawn. On Unix this leaves a zombie until our
/// process exits (academic: this path only ever runs on Windows in
/// production); on Windows the OS reclaims the handle when the
/// launcher exits, same as the WPF equivalent.
pub fn launch_normal(path: &Path, command_line: &str) -> Result<(), GameError> {
    use std::process::Command;

    let mut cmd = Command::new(path);

    // Mirror WPF `WorkingDirectory = Path.GetDirectoryName(gamePath)`.
    // If the path has no parent (e.g. the bare string `"game.exe"`)
    // we fall back to `.` — WPF's `Path.GetDirectoryName` of a bare
    // filename returns `""` which `ProcessStartInfo` treats as
    // current-dir; `Path::new(".")` gives us the same effect in a
    // cross-platform way.
    let workdir: &Path = path.parent().unwrap_or_else(|| Path::new("."));
    cmd.current_dir(workdir);

    // Only forward arguments when non-empty — an empty string would
    // push a spurious empty argv entry (on Unix) or a bare trailing
    // space (via raw_arg on Windows), both harmless in practice but
    // noise-free is cheap.
    if !command_line.is_empty() {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Verbatim append — see the doc comment above for why
            // `.arg(command_line)` breaks games that expect
            // space-separated argv tokens.
            cmd.raw_arg(command_line);
        }
        #[cfg(not(windows))]
        {
            // Non-Windows has no production callers; we just need
            // compile + best-effort behaviour so unit tests can run
            // on Linux / macOS CI.
            cmd.arg(command_line);
        }
    }

    cmd.spawn()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use assert_matches::assert_matches;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    // ---- validate_path --------------------------------------------------

    fn tempfile_with_name(dir: &TempDir, name: &str) -> PathBuf {
        let p = dir.path().join(name);
        std::fs::write(&p, b"stub").unwrap();
        p
    }

    #[test]
    fn validate_path_rejects_empty() {
        assert_matches!(validate_path(Path::new("")), Err(GameError::PathEmpty));
    }

    #[test]
    fn validate_path_rejects_missing() {
        let err = validate_path(Path::new("C:/definitely/not/here/game.exe")).unwrap_err();
        assert_matches!(err, GameError::PathNotFound { .. });
    }

    #[test]
    fn validate_path_accepts_ascii_existing_file() {
        let dir = TempDir::new().unwrap();
        let p = tempfile_with_name(&dir, "MapleStory.exe");
        assert_matches!(validate_path(&p), Ok(()));
    }

    #[test]
    fn validate_path_rejects_non_ascii_traditional_chinese() {
        let dir = TempDir::new().unwrap();
        let p = tempfile_with_name(&dir, "遊戲.exe");
        let err = validate_path(&p).unwrap_err();
        match err {
            GameError::PathNonAscii { offending_char, .. } => {
                assert!(
                    (offending_char as u32) > 128,
                    "offending char must have codepoint > 128"
                );
            }
            other => panic!("expected PathNonAscii, got {other:?}"),
        }
    }

    #[test]
    fn validate_path_rejects_non_ascii_japanese() {
        let dir = TempDir::new().unwrap();
        let p = tempfile_with_name(&dir, "ゲーム.exe");
        let err = validate_path(&p).unwrap_err();
        assert_matches!(err, GameError::PathNonAscii { .. });
    }

    #[test]
    fn validate_path_rejects_non_ascii_emoji() {
        let dir = TempDir::new().unwrap();
        let p = tempfile_with_name(&dir, "game🎮.exe");
        let err = validate_path(&p).unwrap_err();
        assert_matches!(err, GameError::PathNonAscii { .. });
    }

    // ---- GameStartMode::try_from ----------------------------------------

    #[test]
    fn game_start_mode_try_from_parses_auto() {
        assert_eq!(GameStartMode::try_from(0).unwrap(), GameStartMode::Auto);
    }

    #[test]
    fn game_start_mode_try_from_parses_normal() {
        assert_eq!(GameStartMode::try_from(1).unwrap(), GameStartMode::Normal);
    }

    #[test]
    fn game_start_mode_try_from_parses_locale_remulator() {
        assert_eq!(
            GameStartMode::try_from(2).unwrap(),
            GameStartMode::LocaleRemulator
        );
    }

    #[test]
    fn game_start_mode_try_from_clamps_large_values_to_lr() {
        // Mirrors WPF L1863-1864 — any >= 2 falls to LR, not an error.
        assert_eq!(
            GameStartMode::try_from(3).unwrap(),
            GameStartMode::LocaleRemulator
        );
        assert_eq!(
            GameStartMode::try_from(999).unwrap(),
            GameStartMode::LocaleRemulator
        );
    }

    #[test]
    fn game_start_mode_try_from_rejects_negative() {
        assert_eq!(GameStartMode::try_from(-1).unwrap_err(), -1);
    }

    // ---- locale_to_resolved_mode ----------------------------------------

    #[test]
    fn locale_to_resolved_mode_zh_tw_is_normal() {
        assert_eq!(locale_to_resolved_mode("zh-TW"), ResolvedMode::Normal);
    }

    #[test]
    fn locale_to_resolved_mode_zh_hk_is_normal() {
        assert_eq!(locale_to_resolved_mode("zh-HK"), ResolvedMode::Normal);
    }

    #[test]
    fn locale_to_resolved_mode_all_wpf_traditional_chinese_tags_are_normal() {
        for tag in ["zh-Hant", "zh-CHT", "zh-TW", "zh-HK", "zh-MO"] {
            assert_eq!(
                locale_to_resolved_mode(tag),
                ResolvedMode::Normal,
                "tag {tag} must route to Normal per WPF L1842-1847",
            );
        }
    }

    #[test]
    fn locale_to_resolved_mode_en_us_is_lr() {
        assert_eq!(
            locale_to_resolved_mode("en-US"),
            ResolvedMode::LocaleRemulator
        );
    }

    #[test]
    fn locale_to_resolved_mode_zh_cn_is_lr() {
        // Simplified Chinese is deliberately *not* in WPF's case arm
        // — Maple Story TW uses CP950, which the SC system locale
        //   (CP936) can't render correctly.
        assert_eq!(
            locale_to_resolved_mode("zh-CN"),
            ResolvedMode::LocaleRemulator
        );
    }

    #[test]
    fn locale_to_resolved_mode_japanese_is_lr() {
        assert_eq!(
            locale_to_resolved_mode("ja-JP"),
            ResolvedMode::LocaleRemulator
        );
    }

    // ---- resolve_mode ---------------------------------------------------

    #[test]
    fn resolve_mode_normal_is_pass_through() {
        assert_eq!(resolve_mode(GameStartMode::Normal), ResolvedMode::Normal);
    }

    #[test]
    fn resolve_mode_lr_is_pass_through() {
        assert_eq!(
            resolve_mode(GameStartMode::LocaleRemulator),
            ResolvedMode::LocaleRemulator
        );
    }

    #[test]
    fn resolve_mode_auto_returns_a_concrete_mode() {
        // System-dependent: just verify we don't panic and do hand
        // back one of the two concrete arms. On non-Windows this
        // always lands on LocaleRemulator; on Windows it depends on
        // the CI runner's locale.
        let got = resolve_mode(GameStartMode::Auto);
        assert!(matches!(
            got,
            ResolvedMode::Normal | ResolvedMode::LocaleRemulator
        ));
    }

    // ---- substitute_credentials -----------------------------------------

    #[test]
    fn substitute_credentials_two_slots_both_filled() {
        let got = substitute_credentials("/acc:%s /pwd:%s", "user1", "pw1");
        assert_eq!(got, "/acc:user1 /pwd:pw1");
    }

    #[test]
    fn substitute_credentials_single_slot_only_account() {
        // One `%s` in the template → only account is injected, the
        // follow-up `.replacen("%s", password, 1)` is a no-op.
        let got = substitute_credentials("login:%s", "user1", "pw1");
        assert_eq!(got, "login:user1");
    }

    #[test]
    fn substitute_credentials_zero_slots_returns_template_verbatim() {
        let got = substitute_credentials("--no-args", "user1", "pw1");
        assert_eq!(got, "--no-args");
    }

    #[test]
    fn substitute_credentials_empty_template_stays_empty() {
        assert_eq!(substitute_credentials("", "user1", "pw1"), "");
    }

    #[test]
    fn substitute_credentials_empty_account_still_substitutes() {
        // WPF guards at call site (`account != ""`), but the pure
        // helper itself happily produces `"/acc: /pwd:pw1"` — lock
        // that in so a caller-side guard change stays visible.
        let got = substitute_credentials("/acc:%s /pwd:%s", "", "pw1");
        assert_eq!(got, "/acc: /pwd:pw1");
    }

    #[test]
    fn substitute_credentials_three_slots_leaves_third_literal() {
        // Parity lock with WPF's two-pass `Regex.Replace(... , 1)`
        // pattern: the third `%s` is not touched.
        let got = substitute_credentials("%s/%s/%s", "user1", "pw1");
        assert_eq!(got, "user1/pw1/%s");
    }

    // ---- launch_normal --------------------------------------------------

    #[cfg(windows)]
    #[test]
    fn launch_normal_spawns_cmd_exit_zero() {
        // Smoke test: verify API shape + spawn path on the primary
        // target OS. `cmd /c exit 0` returns immediately so the
        // detached child doesn't leak for long.
        let cmd_exe = Path::new(r"C:\Windows\System32\cmd.exe");
        assert!(cmd_exe.exists(), "test requires cmd.exe on the runner");
        launch_normal(cmd_exe, "/c exit 0").expect("spawn must succeed");
    }

    #[test]
    fn launch_normal_returns_spawn_error_for_missing_binary() {
        // Windows' CreateProcess fails fast on a missing file so this
        // produces Err. On Unix, `fork + exec` succeeds the fork then
        // fails the exec inside the child — the parent sees Ok from
        // spawn. Gate accordingly to keep the test deterministic.
        #[cfg(windows)]
        {
            let err = launch_normal(Path::new("NOPE-missing-binary.exe"), "").unwrap_err();
            assert_matches!(err, GameError::Spawn(_));
        }
        #[cfg(not(windows))]
        {
            let _ = launch_normal(Path::new("/nope/missing-binary"), "");
            // No assertion: behaviour differs by kernel and the test
            // only exists to keep non-Windows builds from drifting.
        }
    }
}
