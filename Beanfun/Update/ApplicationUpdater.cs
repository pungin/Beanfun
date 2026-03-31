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
            var url = $"https://api.github.com/repos/lshw54/Beanfun/releases";

            try
            {
                var client = new WebClient();
                // Set User-Agent as required by GitHub API
                client.Headers.Add("User-Agent", $"Beanfun(V{App.AssemblyVersion})");
                client.Headers.Add("Accept", "application/vnd.github.v3+json");
                var json = Encoding.UTF8.GetString(client.DownloadData(url));

                var releases = JsonConvert.DeserializeObject<List<GitHubRelease>>(json);
                GitHubRelease release = GetLastRelease(releases);
                if (release == null)
                    return;

                // Parse Tag Name (e.g., v5.8.2604011114)
                // Groups: [1]=Major, [2]=Minor, [3]=Timestamp/Build
                var match = Regex.Match(release.TagName, @"^v(\d+)\.(\d+)\.(\d+)$");
                if (!match.Success)
                    return;

                // Create a display string for the UI, e.g., "5.8(2604011114)"
                string newVerDisplay =
                    $"{match.Groups[1].Value}.{match.Groups[2].Value}({match.Groups[3].Value})";

                // Compare local version vs remote version using numeric logic
                if (
                    IsNewerVersion(
                        App.AssemblyVersion,
                        match.Groups[1].Value,
                        match.Groups[2].Value,
                        match.Groups[3].Value
                    )
                )
                {
                    try
                    {
                        MessageBoxResult result = MessageBox.Show(
                            string.Format(
                                Regex.Unescape(
                                    Application.Current.TryFindResource("NewVersionDetected")
                                        as string
                                ),
                                newVerDisplay,
                                App.AssemblyVersion,
                                release.Body
                            ),
                            Application.Current.TryFindResource("UpdateCheck") as string,
                            MessageBoxButton.OKCancel
                        );

                        if (result == MessageBoxResult.OK)
                            Process.Start(release.Assets[0].BrowserDownloadUrl);
                    }
                    catch (Exception) { }
                }
                else
                {
                    // If manually checking and no update is found
                    if (show)
                        MessageBox.Show(
                            Application.Current.TryFindResource("NoUpdatesDetected") as string,
                            Application.Current.TryFindResource("UpdateCheck") as string,
                            MessageBoxButton.OK
                        );
                }
            }
            catch (Exception) { }
        }

        private static GitHubRelease GetLastRelease(List<GitHubRelease> releases)
        {
            // Check if user wants Stable channel
            bool stable = ConfigAppSettings.GetValue("updateChannel", "Stable").Equals("Stable");

            foreach (var release in releases)
            {
                // If user is on Beta/Preview channel (!stable),
                // we take the absolute newest release (the first one in the list).
                if (!stable)
                {
                    return release;
                }

                // If user is on Stable channel,
                // we skip any prerelease and only return the first official release found.
                if (!release.Prerelease)
                {
                    return release;
                }
            }
            return null;
        }

        /// <summary>
        /// Compares versions by converting them into a sequence of digits.
        /// Optimized to handle transitions from legacy MS build days to new timestamps.
        /// </summary>
        private static bool IsNewerVersion(
            string localVer,
            string major,
            string minor,
            string timestamp
        )
        {
            try
            {
                // 1. Detect Legacy MS Format (e.g., 5.8.9586...)
                // MS build days are typically 9000+, while your timestamp starts with 26 (year 2026).
                // If local version contains the old MS "Days" pattern, force an update.
                if (localVer.Contains(".9"))
                {
                    return true;
                }

                // 2. Clean local version (e.g., "5.8(2603311757)" -> 582603311757)
                string localDigits = Regex.Replace(localVer, @"[^\d]", "");
                if (!long.TryParse(localDigits, out long localNum))
                    return true; // If local is unreadable, assume it needs update

                // 3. Combine remote parts (e.g., "5" + "8" + "2604011114" -> 582604011114)
                // Ensure minor is padded if you ever hit version 5.10.x
                string remoteDigits = major + int.Parse(minor).ToString() + timestamp;
                if (!long.TryParse(remoteDigits, out long remoteNum))
                    return false;

                // 4. Final Comparison
                // If remote timestamp (260401...) > local timestamp (260331...)
                return remoteNum > localNum;
            }
            catch
            {
                // On error, only update if the local version string looks suspect
                return localVer.Contains(".9");
            }
        }
    }
}
