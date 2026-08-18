using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Security.Cryptography;
using System.Threading.Tasks;
using Microsoft.Win32;
using Newtonsoft.Json.Linq;

namespace Beanfun
{
    /// <summary>
    /// beanfun 台服取密碼時要求附上的 CV / Hash / arch — 也就是「是哪一版
    /// 橘子遊戲大廳（GGM）在問」。缺這三個參數，伺服器只會回
    /// <c>0;Query String Error</c>（issue #368）。
    ///
    /// 值的來源依序：
    ///   1. 使用者自己釘的 %APPDATA%\Beanfun\ggm-client.json（帶 "override": true）
    ///   2. 本機安裝的 GGM 與線上發佈的 ggm-client.json，取版本較新的那份
    ///   3. 編譯進來的常數
    ///
    /// 為什麼不無條件相信本機那份：GGM 會自己更新，但只在它被開起來的時候。
    /// 會用這個登入器的人正是從來不開 GGM 的人，所以放著沒動的安裝回報的
    /// 就是 beanfun 已經不收的舊值 — 最需要救的機器反而是線上熱修永遠碰不到
    /// 的那批。比版本、新的贏（同版本以本機為準），兩種風險都避開。
    ///
    /// 每一層都是盡力而為，失敗就往下一層掉。網路抖一下不該害使用者拿不到
    /// 密碼，畢竟編譯進來的那組就在旁邊。
    /// </summary>
    public class ClientIntegrity
    {
        /// <summary>
        /// 這個分支引用了 log4net 卻從來沒設定過 appender，寫進去等於沒寫，
        /// 所以診斷訊息跟這支檔案其他地方一樣走 Console — 從主控台啟動
        /// （dotnet run）就看得到。
        /// </summary>
        private static void Log(string message)
        {
            Console.WriteLine("[ClientIntegrity] " + message);
        }

        public string CV;
        public string Hash;
        public string Arch;

        /// <summary>沒有本機 GGM 可讀時用的 CV，必須與 FallbackHash 成對。</summary>
        private const string FallbackCV = "1.5.0.2";

        /// <summary>GGM 1.5.0.2 所附 GGMWebStart.dll 的 SHA-256（小寫十六進位）。</summary>
        private const string FallbackHash =
            "dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06";

        private const string GgmDllName = "GGMWebStart.dll";
        private const string CacheFileName = "ggm-client.json";

        /// <summary>
        /// 發佈點，以及 GitHub 連不上時同一份檔案的鏡像（大陸使用者基本上都是）。
        /// </summary>
        private static readonly string[] HotfixUrls =
        {
            "https://raw.githubusercontent.com/pungin/Beanfun/code/ggm-client.json",
            "https://cdn.jsdelivr.net/gh/pungin/Beanfun@code/ggm-client.json",
            "https://fastly.jsdelivr.net/gh/pungin/Beanfun@code/ggm-client.json",
            "https://ghproxy.net/https://raw.githubusercontent.com/pungin/Beanfun/code/ggm-client.json",
        };

        /// <summary>
        /// 快取多久才再抓一次。六小時就是這條熱修線的實際延遲：發錯值要多久才
        /// 止血、發對值要多久才傳到所有人。夠短算修好，夠長不會每次取密碼都拉檔。
        /// </summary>
        private static readonly TimeSpan CacheTtl = TimeSpan.FromHours(6);

        private static readonly TimeSpan FetchTimeout = TimeSpan.FromSeconds(5);

        /// <summary>編譯進來的那組，給兩邊都讀不到的機器。</summary>
        public static ClientIntegrity Fallback()
        {
            return new ClientIntegrity
            {
                CV = FallbackCV,
                Hash = FallbackHash,
                Arch = Environment.Is64BitProcess ? "x64" : "x86",
            };
        }

        public static ClientIntegrity Resolve()
        {
            try
            {
                // 1. 使用者自己釘的。明確選擇壓過一切，包含比較新的線上值。
                var pinned = ReadPinned();
                if (pinned != null)
                {
                    Log("ggm-hotfix: 使用本機釘選的值 cv=" + pinned.CV);
                    return pinned;
                }

                // 2. 本機 GGM 與線上發佈值，取版本較新的。
                var local = ResolveLocal();
                var published = ReadPublished();

                if (local != null && published != null)
                {
                    if (NamesNewerBuild(published.CV, local.CV))
                    {
                        Log(
                            "ggm-hotfix: 線上值比本機 GGM 新 local="
                                + local.CV
                                + " published="
                                + published.CV
                        );
                        return published;
                    }
                    return local;
                }
                if (local != null)
                    return local;
                if (published != null)
                    return published;
            }
            catch (Exception e)
            {
                Log("解析失敗，改用編譯進來的常數: " + e.Message);
            }

            // 3. 編譯進來的那組。
            return Fallback();
        }

        #region 本機 GGM

        /// <summary>本機 GGM 的值；沒有可讀的 GGM 就回 null。</summary>
        private static ClientIntegrity ResolveLocal()
        {
            string dll = LocateGgmDll();
            if (dll == null)
                return null;

            string hash = Sha256LowerHex(dll);
            string cv = FileVersion(dll);
            // CV 跟 Hash 描述的是同一個檔案，混搭會描述出一個不存在的用戶端，
            // 所以有一半讀不到就整組不要。
            if (hash == null || cv == null)
                return null;

            return new ClientIntegrity
            {
                CV = cv,
                Hash = hash,
                Arch = Environment.Is64BitProcess ? "x64" : "x86",
            };
        }

        private static string LocateGgmDll()
        {
            foreach (string dir in GgmDirectories())
            {
                try
                {
                    string candidate = Path.Combine(dir, GgmDllName);
                    if (File.Exists(candidate))
                        return candidate;
                }
                catch { }
            }
            return null;
        }

        /// <summary>候選安裝目錄，最可信的排前面。</summary>
        private static IEnumerable<string> GgmDirectories()
        {
            var dirs = new List<string>();

            // 安裝程式寫的 gamaniagames:// 協定處理器，就算裝在非預設路徑也追得到。
            string fromHandler = GgmDirFromProtocolHandler();
            if (fromHandler != null)
                dirs.Add(fromHandler);

            // 預設安裝位置，涵蓋登錄檔被清掉但檔案還在的情況。
            foreach (string envVar in new[] { "ProgramFiles", "ProgramFiles(x86)" })
            {
                string root = Environment.GetEnvironmentVariable(envVar);
                if (!string.IsNullOrEmpty(root))
                    dirs.Add(Path.Combine(root, "gamania Games", "gamania Games Manager"));
            }
            return dirs;
        }

        /// <summary>
        /// 讀 HKCR\gamaniagames\shell\open\command，值長得像
        /// <c>"C:\...\GGMWebStart.exe" "%1"</c>，取出它的目錄。
        /// </summary>
        private static string GgmDirFromProtocolHandler()
        {
            try
            {
                using (
                    RegistryKey key = Registry.ClassesRoot.OpenSubKey(
                        @"gamaniagames\shell\open\command"
                    )
                )
                {
                    if (key == null)
                        return null;
                    string command = key.GetValue("") as string;
                    if (string.IsNullOrWhiteSpace(command))
                        return null;

                    command = command.Trim();
                    string exe;
                    if (command.StartsWith("\""))
                    {
                        int end = command.IndexOf('"', 1);
                        if (end <= 1)
                            return null;
                        exe = command.Substring(1, end - 1);
                    }
                    else
                    {
                        exe = command.Split(' ')[0];
                    }
                    return string.IsNullOrEmpty(exe) ? null : Path.GetDirectoryName(exe);
                }
            }
            catch
            {
                return null;
            }
        }

        private static string Sha256LowerHex(string path)
        {
            try
            {
                using (var sha = SHA256.Create())
                using (var stream = File.OpenRead(path))
                {
                    return BitConverter
                        .ToString(sha.ComputeHash(stream))
                        .Replace("-", "")
                        .ToLowerInvariant();
                }
            }
            catch
            {
                return null;
            }
        }

        /// <summary>
        /// 讀 Win32 版本資源的 FileVersion。GGM 送的是組件版本，但目前看過的
        /// 每個 GGM 版本兩者都一致；真的分歧時上層會掉回編譯常數。
        /// </summary>
        private static string FileVersion(string path)
        {
            try
            {
                var info = FileVersionInfo.GetVersionInfo(path);
                if (info.FileMajorPart == 0 && info.FileMinorPart == 0 && info.FileBuildPart == 0)
                    return null;
                return string.Format(
                    "{0}.{1}.{2}.{3}",
                    info.FileMajorPart,
                    info.FileMinorPart,
                    info.FileBuildPart,
                    info.FilePrivatePart
                );
            }
            catch
            {
                return null;
            }
        }

        #endregion

        #region 線上發佈值

        private static string CachePath()
        {
            return Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                "Beanfun",
                CacheFileName
            );
        }

        /// <summary>
        /// 使用者釘的值。用 "override": true 這個旗標分辨，而不是換個檔案放 —
        /// 直接改抓下來的那份就能釘住，不必解釋第二條路徑，也不會被下次抓檔蓋掉。
        /// </summary>
        private static ClientIntegrity ReadPinned()
        {
            try
            {
                string path = CachePath();
                if (!File.Exists(path))
                    return null;
                string body = File.ReadAllText(path);
                var json = JObject.Parse(StripBom(body));
                if (json.Value<bool?>("override") != true)
                    return null;
                return Parse(body);
            }
            catch
            {
                return null;
            }
        }

        /// <summary>快取還新鮮就用快取，否則重抓；抓不到就用舊的那份。</summary>
        private static ClientIntegrity ReadPublished()
        {
            try
            {
                string path = CachePath();
                if (File.Exists(path))
                {
                    var cached = Parse(File.ReadAllText(path));
                    if (cached != null && DateTime.UtcNow - File.GetLastWriteTimeUtc(path) < CacheTtl)
                        return cached;

                    var refreshed = Fetch();
                    if (refreshed != null)
                        return refreshed;
                    // 舊的也比沒有好：它至少曾經好到被發佈出來，而替代品是
                    // 編譯進來那組，照定義更舊。
                    return cached;
                }
                return Fetch();
            }
            catch
            {
                return null;
            }
        }

        /// <summary>
        /// 這次執行已經整輪抓失敗過。連不上任何鏡像的人（例如整條 GitHub 都被
        /// 擋掉）每次取密碼都要先卡滿四次逾時，而答案不會變，所以一次執行只試
        /// 一輪。重開程式就會再試。
        /// </summary>
        private static bool fetchFailedThisRun;

        private static ClientIntegrity Fetch()
        {
            if (fetchFailedThisRun)
                return null;

            foreach (string url in HotfixUrls)
            {
                try
                {
                    string body;
                    using (var http = new HttpClient { Timeout = FetchTimeout })
                    {
                        body = Task.Run(() => http.GetStringAsync(url)).GetAwaiter().GetResult();
                    }
                    var values = Parse(body);
                    if (values == null)
                    {
                        // 連得到但不能用，值得記是哪個鏡像：CDN 快取過期看起來
                        // 跟 commit 發錯值一模一樣。
                        Log("ggm-hotfix: 發佈檔驗證未過 " + url);
                        continue;
                    }
                    WriteCache(body);
                    Log("ggm-hotfix: 已取得發佈值 cv=" + values.CV + " from " + url);
                    return values;
                }
                catch { }
            }
            fetchFailedThisRun = true;
            Log("ggm-hotfix: 沒有鏡像回應，改用本機來源");
            return null;
        }

        private static void WriteCache(string body)
        {
            try
            {
                string path = CachePath();
                Directory.CreateDirectory(Path.GetDirectoryName(path));
                File.WriteAllText(path, body);
            }
            catch (Exception e)
            {
                Log("ggm-hotfix: 無法寫入快取: " + e.Message);
            }
        }

        /// <summary>
        /// 去掉 UTF-8 BOM。Windows 上不少編輯器存檔會加，而 BOM 會讓解析失敗 —
        /// 那是熱修最糟的失敗方式：看起來發佈了，實際上每個人都掉回舊常數。
        /// </summary>
        private static string StripBom(string body)
        {
            return body != null && body.Length > 0 && body[0] == '\uFEFF' ? body.Substring(1) : body;
        }

        /// <summary>
        /// 解析並驗證發佈檔。驗證不是客氣：格式壞掉的值會被當成我們的身分送給
        /// beanfun，然後所有人被拒。不像版本號和 SHA-256 的一律視同檔案不存在。
        /// </summary>
        private static ClientIntegrity Parse(string body)
        {
            try
            {
                var json = JObject.Parse(StripBom(body));
                string cv = (json.Value<string>("cv") ?? "").Trim();
                string hash = (json.Value<string>("hash") ?? "").Trim().ToLowerInvariant();
                if (!IsVersion(cv) || !IsSha256(hash))
                {
                    Log("ggm-hotfix: 值未通過驗證 cv=" + cv + " hash_len=" + hash.Length);
                    return null;
                }
                return new ClientIntegrity
                {
                    CV = cv,
                    Hash = hash,
                    // arch 從不發佈：它描述的是誰在問，也就是這支程式，不是產出
                    // 這組值的那台機器。
                    Arch = Environment.Is64BitProcess ? "x64" : "x86",
                };
            }
            catch
            {
                return null;
            }
        }

        private static bool IsVersion(string cv)
        {
            if (string.IsNullOrEmpty(cv))
                return false;
            bool hasDigit = false;
            foreach (char c in cv)
            {
                if (c >= '0' && c <= '9')
                    hasDigit = true;
                else if (c != '.')
                    return false;
            }
            return hasDigit;
        }

        private static bool IsSha256(string hash)
        {
            if (hash == null || hash.Length != 64)
                return false;
            foreach (char c in hash)
            {
                bool hex = (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f');
                if (!hex)
                    return false;
            }
            return true;
        }

        #endregion

        /// <summary>
        /// candidate 是否比 current 新。逐段比數字，缺的段當 0，所以 1.5.1 大於
        /// 1.5；解析不出來的當 0，讓格式壞掉的那邊輸 — 輸了只是不被採用，安全。
        /// </summary>
        internal static bool NamesNewerBuild(string candidate, string current)
        {
            string[] a = (candidate ?? "").Split('.');
            string[] b = (current ?? "").Split('.');
            int width = Math.Max(a.Length, b.Length);
            for (int i = 0; i < width; i++)
            {
                long x = SegmentAt(a, i);
                long y = SegmentAt(b, i);
                if (x != y)
                    return x > y;
            }
            return false;
        }

        private static long SegmentAt(string[] parts, int index)
        {
            long value;
            if (index >= parts.Length || !long.TryParse(parts[index].Trim(), out value))
                return 0;
            return value;
        }
    }
}
