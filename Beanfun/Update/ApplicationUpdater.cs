using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Net;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows;
using Newtonsoft.Json;

namespace Beanfun.Update
{
    class ApplicationUpdater
    {
        public class GitHubRelease
        {
            [JsonProperty("name")]
            public string Name { get; set; }

            [JsonProperty("tag_name")]
            public string TagName { get; set; }

            [JsonProperty("prerelease")]
            public bool Prerelease { get; set; }

            [JsonProperty("body")]
            public string Body { get; set; }

            [JsonProperty("assets")]
            public List<GitHubAsset> Assets { get; set; }
        }

        public class GitHubAsset
        {
            [JsonProperty("browser_download_url")]
            public string BrowserDownloadUrl { get; set; }
        }

        internal static void CheckApplicationUpdate(bool show)
        {
            var url = "https://api.github.com/repos/lshw54/Beanfun/releases";

            try
            {
                using (var client = new WebClient())
                {
                    client.Headers.Add("User-Agent", $"Beanfun(V{App.AssemblyVersion})");
                    client.Headers.Add("Accept", "application/vnd.github.v3+json");
                    var json = Encoding.UTF8.GetString(client.DownloadData(url));

                    var releases = JsonConvert.DeserializeObject<List<GitHubRelease>>(json);
                    GitHubRelease release = GetLastRelease(releases);

                    if (release == null)
                        return;

                    // 1. 解析遠端 Tag (支援 v5.8.3.2604011114)
                    // Groups: [1]=Major, [2]=Minor, [3]=Patch, [4]=Timestamp
                    var match = Regex.Match(release.TagName, @"^v(\d+)\.(\d+)\.(\d+)\.(\d+)$");
                    if (!match.Success)
                        return;

                    string major = match.Groups[1].Value;
                    string minor = match.Groups[2].Value;
                    string patch = match.Groups[3].Value;
                    string timestamp = match.Groups[4].Value;

                    // 2. 準備顯示文字
                    string newVerDisplay = $"{major}.{minor}.{patch}({timestamp})";

                    // 3. 數值比較邏輯
                    if (IsNewerVersion(App.AssemblyVersion, major, minor, timestamp))
                    {
                        string msg = string.Format(
                            Regex.Unescape(
                                Application.Current.TryFindResource("NewVersionDetected") as string
                                    ?? "Detect New Version {0} (Current: {1})\n\n{2}"
                            ),
                            newVerDisplay,
                            App.AssemblyVersion,
                            release.Body
                        );

                        MessageBoxResult result = MessageBox.Show(
                            msg,
                            Application.Current.TryFindResource("UpdateCheck") as string
                                ?? "Update Check",
                            MessageBoxButton.OKCancel
                        );

                        if (result == MessageBoxResult.OK)
                        {
                            string downloadUrl =
                                (release.Assets != null && release.Assets.Count > 0)
                                    ? release.Assets[0].BrowserDownloadUrl
                                    : $"https://github.com/lshw54/Beanfun/releases/tag/{release.TagName}";

                            Process.Start(
                                new ProcessStartInfo
                                {
                                    FileName = downloadUrl,
                                    UseShellExecute = true,
                                }
                            );
                        }
                    }
                    else if (show)
                    {
                        MessageBox.Show(
                            Application.Current.TryFindResource("NoUpdatesDetected") as string
                                ?? "No Updates Found",
                            Application.Current.TryFindResource("UpdateCheck") as string
                                ?? "Update Check",
                            MessageBoxButton.OK
                        );
                    }
                }
            }
            catch (Exception ex)
            {
                Debug.WriteLine("Update check failed: " + ex.Message);
            }
        }

        private static GitHubRelease GetLastRelease(List<GitHubRelease> releases)
        {
            string channel = ConfigAppSettings.GetValue("updateChannel", "Stable");
            bool isBeta = channel.Equals("Beta") || channel.Equals("Preview");

            foreach (var release in releases)
            {
                if (isBeta)
                    return release; // Beta 頻道攞絕對最新
                if (!release.Prerelease)
                    return release; // Stable 頻道跳過 Prerelease
            }
            return null;
        }

        private static bool IsNewerVersion(
            string localVer,
            string major,
            string minor,
            string timestamp
        )
        {
            try
            {
                // 1. 遠端版本標準化 (e.g. 0050082604011114)
                long remoteNum = long.Parse(
                    string.Format("{0:D3}{1:D3}{2}", int.Parse(major), int.Parse(minor), timestamp)
                );

                // 2. 本地版本標準化
                long localNum;
                // Regex 修正：\.? 兼容 5.8.(xxx) 或者 5.8(xxx) 格式
                var match = Regex.Match(localVer, @"(\d+)\.(\d+)\.?\((\d+)\)");

                if (match.Success)
                {
                    localNum = long.Parse(
                        string.Format(
                            "{0:D3}{1:D3}{2}",
                            int.Parse(match.Groups[1].Value),
                            int.Parse(match.Groups[2].Value),
                            match.Groups[3].Value
                        )
                    );
                }
                else if (Version.TryParse(localVer, out Version v))
                {
                    // 兼容舊版 5.8.9586... (通常會細過 16 位 Timestamp 數值)
                    localNum = long.Parse(
                        string.Format("{0:D3}{1:D3}{2}{3}", v.Major, v.Minor, v.Build, v.Revision)
                    );
                }
                else
                {
                    // 暴力提取所有數字
                    localNum = long.Parse(Regex.Replace(localVer, @"[^\d]", ""));
                }

                // 3. 嚴格大於才更新
                return remoteNum > localNum;
            }
            catch
            {
                return false;
            }
        }
    }
}
