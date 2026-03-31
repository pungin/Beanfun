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
            bool stable = ConfigAppSettings.GetValue("updateChannel", "Stable").Equals("Stable");
            foreach (var release in releases)
            {
                // If Beta channel, return the first release (could be prerelease)
                if (!stable && release.Prerelease)
                    return release;
                // If Stable channel, skip prereleases
                else if (!release.Prerelease)
                    return release;
            }
            return null;
        }

        /// <summary>
        /// Compares versions by converting them into a continuous sequence of digits.
        /// This avoids System.Version overflow issues with long timestamps.
        /// </summary>
        /// <param name="localVer">The current local version string, e.g., "5.8(2604011114)"</param>
        /// <param name="major">Remote major version</param>
        /// <param name="minor">Remote minor version</param>
        /// <param name="timestamp">Remote timestamp/build part</param>
        /// <returns>True if remote version is strictly greater than local version</returns>
        private static bool IsNewerVersion(
            string localVer,
            string major,
            string minor,
            string timestamp
        )
        {
            try
            {
                // 1. Clean local version: remove dots, parentheses, etc.
                // "5.8(2604011114)" -> 582604011114
                string localDigits = Regex.Replace(localVer, @"[^\d]", "");
                if (!long.TryParse(localDigits, out long localNum))
                    return false;

                // 2. Combine remote parts into a single numeric string
                // "5" + "8" + "2604011114" -> 582604011114
                string remoteDigits = major + minor + timestamp;
                if (!long.TryParse(remoteDigits, out long remoteNum))
                    return false;

                // 3. Simple numeric comparison
                return remoteNum > localNum;
            }
            catch
            {
                // Fallback to no update on parsing error
                return false;
            }
        }
    }
}
