# Beanfun

[![GitHub all releases](https://img.shields.io/github/downloads/pungin/Beanfun/total)](https://github.com/pungin/Beanfun/releases)
[![Format Check](https://github.com/pungin/Beanfun/actions/workflows/format-check.yml/badge.svg)](https://github.com/pungin/Beanfun/actions/workflows/format-check.yml)

>  **遊戲橘子數位科技旗下遊戲的第三方啟動器**

⚠️ **免責聲明：** 本程式 **不是** 遊戲橘子數位科技開發的官方客戶端程式。關於遊戲帳號使用第三方的方式登入，請再三斟酌，並請確認您下載當前程式的途徑是否安全。

* 本程式使用部分 `BeanfunLogin` 的代碼。
* 程式使用 [Locale_Remulator](https://github.com/InWILL/Locale_Remulator) 作為區域模擬元件，支持 32-bit 和 64-bit 遊戲。

---

## 下載與使用 (Getting Started)

### 系統要求 (Prerequisites)
* **作業系統：** Windows 10 或以上
* **必要元件：** [Microsoft Visual C++ Redistributable](https://docs.microsoft.com/zh-CN/cpp/windows/latest-supported-vc-redist?view=msvc-170)

### 使用方法 (Usage)
1. 前往 **[最新發行版 (Releases)](https://github.com/pungin/Beanfun/releases/latest)** 下載最新的 `Beanfun.exe`。
2. 下載後放在任意全英文路徑的資料夾，直接運行即可。

> 💡 **運作原理說明：** 啟動遊戲時，程式會在當前資料夾釋放 `LRProc.dll` 和 `LRHookx32.dll` 或 `LRHookx64.dll` 文件。
> * `LRProc.dll` - 將 Hook dll 載入到遊戲中
> * `LRHookx32.dll` 或 `LRHookx64.dll` - 區域模擬元件

---

## 技術棧 (Built With)

* **[.NET 8](https://dotnet.microsoft.com/en-us/download/dotnet/8.0)** - 目標框架（Self-contained，使用者不需另外安裝 Runtime）
* **[ini-parser-netstandard](https://github.com/lukazh/ini-parser-standard)** - ini 設定檔元件
* **[log4net](https://logging.apache.org/log4net/)** - 日誌記錄元件
* **[Newtonsoft.Json](https://www.newtonsoft.com/json)** - JSON 解析元件
* **[Microsoft.Web.WebView2](https://www.nuget.org/packages/Microsoft.Web.WebView2)** - 內嵌瀏覽器元件
* **[Detours](https://github.com/microsoft/Detours)** - 用於 Hook ANSI/Unicode 函數
* **[Locale_Remulator](https://github.com/InWILL/Locale_Remulator)** - 區域模擬元件

---

## 維護與貢獻規範 (Maintenance & Contribution)

為了確保專案品質以及自動化版本控制的穩定性，所有貢獻者請遵循以下標準開發流程：

### 流程：Fork ➔ Test ➔ PR ➔ Format ➔ Approve

1.  **Fork 專案**：將本倉庫 Fork 到你個人的 GitHub 帳號下進行開發。
2.  **本地測試 (Local Test)**：
    * 在完成功能開發或 Bug 修復後，務必在本地環境進行編譯與功能測試。
    * 確保程式能正常啟動，且不會影響現有的登入功能。
3.  **提交 Pull Request (PR)**：
    * 將你的改動 Push 到你 Fork 的倉庫，並向本專案發起 PR。在 PR 描述中清晰說明你修改的內容與原因。
4.  **執行程式碼格式化 (CSharpier) [強制]**：
    * 提交前請務必執行 CSharpier 確保代碼風格一致，否則 CI 檢查可能會失敗。
    ```bash
    dotnet tool restore
    dotnet csharpier format .
    ```
5.  **審核與合併 (Approve & Merge)**：
    * 維護者審核通過並合併 PR 後，系統會自動接手後續的版號遞增與發布。

---

## 開發與發佈 (Development & Release)

### 全自動化發佈流程 (CI/CD)

> 💡 **核心原則**：本專案的版本發行 **完全零人工作業**。開發者**不需要**在本地自行編譯、打包或手動建立 Release。所有的版本號運算、資訊寫入 (InformationalVersion)、執行檔建置與雙語 Release Notes 的生成，均由 GitHub Actions 全自動處理。

若要發佈新版本，請直接至 GitHub 的 [Actions 頁面](../../actions/workflows/build-and-release.yml) 手動觸發 **Build and Release** workflow 即可。

#### 發佈參數說明

| 參數 | 說明 | 預設值 |
|------|------|--------|
| `release_type` | `release`（正式版）或 `prerelease`（測試版） | `prerelease` |
| `version_increment` | 版本遞增方式：`patch` / `minor` / `major` | `patch` |
| `release_name` | 自訂發佈名稱（留空將由系統自動產生） | 空 |

#### 自動化版本控制機制

系統會將完整的版號（包含 Patch 與精準的 UTC Timestamp）**強制注入**至執行檔中，確保程式內顯示的版號與 GitHub 完全一致。Tag 格式為 `v{major}.{minor}.{patch}.{timestamp}`（例如 `v5.8.13.2603311234`）。

| 操作 | 觸發情境範例 | AssemblyInfo.cs 變化 |
|------|------|-----------------|
| patch | v5.8.12 → v5.8.13 | 自動更新為 5.8.13.* |
| minor | v5.8.13 → v5.9.0 | 自動更新為 5.9.0.* |
| major | v5.9.0 → v6.0.0 | 自動更新為 6.0.0.* |

* **智慧遞增**：Patch 值會自動從 Git 最新 Tag 解析並 +1。
* **Release Notes 生成**：自動抓取版本間的 Commits，排版成包含中英雙語、支援摺疊技術細節的 Release 頁面。
* **自動 Commit**：產生新 Tag 後，系統會自動將修改後的 `AssemblyInfo.cs` Commit 並 Push 回儲存庫。

### 本地測試打包 (僅供開發除錯用)

若你需要在本地端測試打包流程，可使用以下指令。
*(⚠️ 注意：此產出的 `Beanfun.exe` 僅供本地除錯，切勿手動上傳至 GitHub Release)*

本專案使用 [CSharpier](https://csharpier.com/) 作為程式碼格式化工具。

```bash
# 安裝還原工具
dotnet tool restore

# 格式化所有 .cs 檔案
dotnet csharpier format .

# 檢查格式（不修改檔案）
dotnet csharpier check .
```

### 本地測試打包 (僅供開發除錯用)

若你需要在本地端測試打包流程，可使用以下指令。
*(⚠️ 注意：此產出的 `Beanfun.exe` 僅供本地除錯，切勿手動上傳至 GitHub Release)*

```bash
# 清理專案
dotnet clean Beanfun/Beanfun.csproj -c Release

# 建置單一 exe（Self-contained）
dotnet publish Beanfun/Beanfun.csproj -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:IncludeAllContentForSelfExtract=true -p:EnableCompressionInSingleFile=true -p:DebugType=none -o publish
```
