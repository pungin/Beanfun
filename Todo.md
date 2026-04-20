# Beanfun Todo

## 已完成

### F. In-app browser fixes (P12.4-followup-B-fix)

- [x] F1 backend allowlist 改 `*.beanfun.com` suffix match
- [x] F2 砍 VideoReport button（目標 URL 長期 404）
- [x] F3-redo mirror WPF L494 game image URL passthrough + unified fallback host
- [x] F6 ManageAccount 補「上一頁」按鈕
- [x] F7 WebviewWindow 開窗黑屏修復（visible(false) + on_page_load show + 5s safety net）
- [x] F8 客服中心按鈕 wire-up（frontend region → static URL → useInAppBrowser）
- [x] F9 會員中心按鈕 wire-up（backend command, web_token 不離開 Rust）

### A. About 頁修改

- [x] A1 作者署名改為 `By Pungin and YCC3741`
- [x] A2 GitHub 連結改為 `https://github.com/pungin/Beanfun`
- [x] A3 發送電郵拆成兩個垂直排列連結：✉ Pungin / ✉ YCC3741

### B. 最小化到通知中心（system tray）

- [x] B1 i18n: zh-TW `最小化到通知中心` / zh-CN `最小化到通知中心`
- [x] B2 Cargo.toml 加 `tray-icon` feature
- [x] B3 lib.rs 加 `.setup()` 建立 TrayIconBuilder + `on_window_event` minimize 攔截
- [x] B4 新增 `tray.rs` — `build_tray()` + `handle_minimize_to_tray()` + async config 讀取
- [x] B5 tray click：左鍵 → show + unminimize + focus + hide tray

### C. 視窗 / 登入頁微調

- [x] C1 預設視窗高度 600 → 720（消除 scroll）
- [x] C2 登入頁標題 `繽放 Next` → 空心 icon + `繽放`（三語同步）
- [x] C3 登入頁 icon 改用空心描邊版
- [x] C4 地區圖示 Flag → 灰粗圓潤 TW / HK 字樣
- [x] C5 版號 0.1.0 → 6.0.0（Cargo.toml + tauri.conf.json + package.json）

### D. PostMessageW 修正 + updater fix

- [x] D1 `windows-app-manifest.xml` + `build.rs`：release 建置加 `highestAvailable`（mirror WPF manifest）
- [x] D2 登入頁 icon 比例修正（移除固定 width/height，改 height:36px + width:auto）
- [x] D3 updater: GitHub release JSON `body: null` 導致 decode 失敗 → `null_as_empty` deserializer
- [x] D4 updater: 純 semver `6.0.0` 掉入 Path B 誤判有新版 → 加 Path C semver 比較
- [x] D5 視窗寬度 800 → 720

### E. Repo 目錄重組

- [x] E1 `git mv Beanfun/LocaleRemulator` → `src-tauri/LocaleRemulator`
- [x] E2 `git mv Beanfun/Lang` → `Lang/`
- [x] E3 更新 `build.rs` / `locale_remulator.rs` / `convert-lang.mjs` 路徑
- [x] E4 `git rm -r Beanfun/` + `Beanfun.sln` + `.config/` + `stitch-prompt.md`
- [x] E5 扁平化 `beanfun-next/` 到 repo 根
- [x] E6 合併 `.gitignore`，移除 WPF 條目
- [x] E7 CI: 重命名 `ci.yml`，移除 `format-check.yml` + `build-and-release.yml`
- [x] E8 驗證：cargo check/test + npm typecheck/lint/vitest 全通過

### F2. Post-restructure cleanup

- [x] F2.1 App 名稱 `beanfun-next` → `Beanfun`（Cargo.toml / tauri.conf.json / package.json / window title）
- [x] F2.2 Rust lib name `beanfun_next_lib` → `beanfun_lib`（全檔案更新）
- [x] F2.3 CI workflows 檢查（ci.yml / commitlint.yml 結構正確）
- [x] F2.4 `.gitignore` 清理
- [x] F2.5 `README.md` 重寫（移除過時路徑和描述）
- [x] F2.6 `include_bytes!` 路徑修正（`locale_remulator.rs`）
- [x] F2.7 全域 `beanfun-next` 殘留引用掃描 + 修正（comments / tests / configs）
- [x] F2.8 `cargo fmt` + `prettier --write` 格式化
- [x] F2.9 驗證全通過：cargo check ✓ / cargo test (722 passed) ✓ / typecheck ✓ / lint ✓ / vitest (586 passed) ✓

## 待處理（下一輪）

- [ ] CI / 打包 exe 流程

## Smoke test 紀錄

- [x] T1 About: 署名 / GitHub 連結 / 雙 email 各開一次
- [x] T2 Tray: 勾選 → minimize → tray 出現 → 左鍵還原 → tray 消失
- [x] T3 Tray: 不勾 → minimize → 正常縮到 taskbar
- [x] T4 登入頁空心 icon + 繽放 + 視窗不再有 scroll
- [x] T5 地區選擇頁：TW / HK 粗體字取代 Flag icon
- [x] T6 About 頁版號顯示 6.0.0
- [x] T7 回歸: PlayerReport / 會員中心 / 客服中心 / game images

## Commit 紀錄

- [x] `97ff386` feat(next): in-app browser fixes, UI polish, tray, updater & manifest
