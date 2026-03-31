# Beanfun

[![GitHub all releases](https://img.shields.io/github/downloads/pungin/Beanfun/total)](https://github.com/pungin/Beanfun/releases)
[![Format Check](https://github.com/pungin/Beanfun/actions/workflows/format-check.yml/badge.svg)](https://github.com/pungin/Beanfun/actions/workflows/format-check.yml)

遊戲橘子數位科技旗下遊戲的啟動器

本程式**不是**遊戲橘子數位科技開發的官方客戶端程式

關於遊戲帳號使用第三方的方式登入請再三斟酌並且請確認您下載當前程式的途徑是否安全

程式使用部分BeanfunLogin的代碼

程式使用<https://github.com/InWILL/Locale_Remulator>作為區域模擬元件，支持32bit和64bit遊戲

## Download

Download available at <https://github.com/pungin/Beanfun/releases/latest>.

## Getting Started

### Prerequisites

* Windows 10 以上
* [Microsoft Visual C++ Redistributable](https://docs.microsoft.com/zh-CN/cpp/windows/latest-supported-vc-redist?view=msvc-170)

### Usage

下載`Beanfun.exe`後直接運行即可

啟動遊戲時會在當前資料夾釋放`LRProc.dll`和`LRHookx32.dll`或`LRHookx64.dll`文件
* `LRProc.dll` - 將`LRHookx32.dll`或`LRHookx64.dll`載入到遊戲中
* `LRHookx32.dll`或`LRHookx64.dll` - 區域模擬元件

## Built With

* [.NET 8](https://dotnet.microsoft.com/en-us/download/dotnet/8.0) - 目標框架（Self-contained，使用者不需另外安裝）
* [ini-parser-netstandard](https://github.com/lukazh/ini-parser-standard) - ini元件
* [log4net](https://logging.apache.org/log4net/) - 日誌元件
* [Newtonsoft.Json](https://www.newtonsoft.com/json) - JSON元件
* [Microsoft.Web.WebView2](https://www.nuget.org/packages/Microsoft.Web.WebView2) - 內嵌瀏覽器元件
* [Detours](https://github.com/microsoft/Detours) - Used to hook ANSI/Unicode functions
* [Locale_Remulator](https://github.com/InWILL/Locale_Remulator) - 區域模擬元件

## Development

### 建置發行版本

```bash
# 清理並發行單一 exe（Self-contained，不需安裝 .NET Runtime）
dotnet clean Beanfun/Beanfun.csproj -c Release
dotnet publish Beanfun/Beanfun.csproj -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:IncludeAllContentForSelfExtract=true -p:EnableCompressionInSingleFile=true -p:DebugType=none -o publish
```

產出的 `publish/Beanfun.exe` 即為可直接發行的單一執行檔。

### 發佈流程

透過 GitHub Actions 的 **Build and Release** workflow 進行發佈，於 [Actions 頁面](../../actions/workflows/build-and-release.yml) 手動觸發。

#### 參數說明

| 參數 | 說明 | 預設值 |
|------|------|--------|
| `release_type` | `release`（正式版）或 `prerelease`（測試版） | `prerelease` |
| `version_increment` | 版本遞增方式：`patch` / `minor` / `major` | `patch` |
| `release_name` | 自訂發佈名稱（留空自動產生） | 空 |

#### 版本格式

Tag 格式為 `v{major}.{minor}.{patch}.{timestamp}`，例如 `v5.7.2.2603311234`。

| 操作 | 範例 | AssemblyInfo.cs |
|------|------|-----------------|
| patch | v5.7.1 → v5.7.2 | 不變 |
| minor | v5.7.2 → v5.8.0 | 更新為 5.8.* |
| major | v5.8.0 → v6.0.0 | 更新為 6.0.* |

- Patch 值自動從最新的 git tag 解析並遞增
- Release name 留空時自動產生，例如 `Release v5.7.2` 或 `Pre-Release v5.7.2`
- `prerelease` 模式不會修改 AssemblyInfo.cs，僅遞增 patch

### 程式碼格式化

本專案使用 [CSharpier](https://csharpier.com/) 作為程式碼格式化工具。

```bash
# 安裝還原工具
dotnet tool restore

# 格式化所有 .cs 檔案
dotnet csharpier format .

# 檢查格式（不修改檔案）
dotnet csharpier check .
```