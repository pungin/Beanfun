# .NET Framework 4.7.2 → .NET 8 升級 Todo

## Phase 1：csproj 轉換與套件遷移
- [x] 1. 建立新的 SDK-style `Beanfun.csproj`（`net8.0-windows`），包含 `<UseWPF>` 和 `<UseWindowsForms>`
- [x] 2. 將 `packages.config` 的 NuGet 套件改為 `<PackageReference>` 格式
- [x] 3. 新增 `System.Configuration.ConfigurationManager` NuGet 套件（ConfigAppSettings 需要）
- [x] 4. 移除 `Microsoft.mshtml` 參考（程式碼中未使用）
- [x] 5. 保留嵌入資源（LocaleRemulator DLLs、圖片、字型等）
- [x] 6. 設定 Self-contained + Single-file publish 屬性

## Phase 2：修正過時/不相容的 API
- [x] 7. `WCDESComp.cs`：`DESCryptoServiceProvider` → `DES.Create()`
- [x] 8. `App.xaml.cs`：`MD5CryptoServiceProvider` → `MD5.Create()`
- [x] 9. `App.xaml.cs`：`HashAlgorithm.Create()` → `MD5.Create()`（明確指定演算法）
- [x] 10. `AccRecovery.xaml.cs`：`RijndaelManaged` → `Aes.Create()`
- [x] 11. `AccountManager.cs`：`BinaryFormatter` → `Newtonsoft.Json`（**Breaking：舊版帳號資料不相容**）

## Phase 3：移除自訂 AssemblyResolve 機制
- [x] 12. 移除 `App.xaml.cs` 中的 `AssemblyResolve` 事件處理
- [x] 13. 移除 `csproj` 中的 `AfterResolveReferences` target（不再需要嵌入所有 DLL）
- [x] 14. 保留 `ReleaseResource` 方法（LocaleRemulator 仍在使用）

## Phase 4：建置與修正
- [x] 15. 新增 `System.Management` NuGet 套件
- [x] 16. 新增 `System.Drawing.Common` NuGet 套件
- [x] 17. 移除不需要的舊設定（BootstrapperPackage、ClickOnce 等）
- [x] 18. 清理 `app.manifest`（移除 CAS 區塊）
- [x] 19. 刪除 `packages.config` 和 `App.config`

## Phase 5：驗證
- [x] 20. Debug build 成功（0 errors）
- [x] 21. Release build 成功（0 errors）
- [x] 22. Single-file publish 成功（Self-contained: 157.72MB）
