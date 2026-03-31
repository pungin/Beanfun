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
                var match = Regex.Match(release.TagName, @"^v(\d+)\.(\d+)\.(\d+)\.(\d+)$");
                if (!match.Success)
                    return;

                // Create a display string for the UI, e.g., "5.8(2604011114)"
                string major = match.Groups[1].Value;
                string minor = match.Groups[2].Value;
                string patch = match.Groups[3].Value;
                string timestamp = match.Groups[4].Value;
                string newVerDisplay = $"{major}.{minor}.{patch}({timestamp})";

                // Compare local version vs remote version using numeric logic
                if (IsNewerVersion(App.AssemblyVersion, major, minor, timestamp))
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
                        {
                            try
                            {
                                // 1. Check if there are any assets available
                                if (release.Assets != null && release.Assets.Count > 0)
                                {
                                    string downloadUrl = release.Assets[0].BrowserDownloadUrl;

                                    // 2. For .NET Core / .NET 5+, Process.Start needs UseShellExecute = true to open URLs
                                    Process.Start(
                                        new ProcessStartInfo
                                        {
                                            FileName = downloadUrl,
                                            UseShellExecute = true,
                                        }
                                    );
                                }
                                else
                                {
                                    // Fallback: If assets are missing, open the release page instead
                                    // Using the tag_name to construct the URL
                                    string releasePage =
                                        $"https://github.com/lshw54/Beanfun/releases/tag/{release.TagName}";
                                    Process.Start(
                                        new ProcessStartInfo
                                        {
                                            FileName = releasePage,
                                            UseShellExecute = true,
                                        }
                                    );
                                }
                            }
                            catch (Exception ex)
                            {
                                // Log or show error if needed
                                Debug.WriteLine("Failed to open download link: " + ex.Message);
                            }
                        }
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
        /// Compares versions by converting them into a normalized long integer.
        /// Uses padding to ensure semantic versioning (Major.Minor) remains consistent.
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
                // 1. Normalize Remote Version: Major(3 digits) + Minor(3 digits) + Timestamp(10 digits)
                // e.g. 5, 8, 2604011114 -> 0050082604011114
                long remoteNum = long.Parse(
                    string.Format("{0:D3}{1:D3}{2}", int.Parse(major), int.Parse(minor), timestamp)
                );

                // 2. Normalize Local Version
                // We extract ALL digits and check the format
                string localDigitsOnly = Regex.Replace(localVer, @"[^\d]", "");

                // If it's the old format "5.8.9586.32322", it might have 4 segments
                // We try to parse the local string as a Version object first to be safe
                Version v;
                long localNum;

                if (localVer.Contains("(") && localVer.Contains(")"))
                {
                    // New format: "5.8(2603311757)"
                    // Extract Major, Minor and the stuff inside brackets
                    var match = Regex.Match(localVer, @"(\d+)\.(\d+)\((\d+)\)");
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
                    else
                    {
                        localNum = long.Parse(localDigitsOnly);
                    }
                }
                else if (Version.TryParse(localVer, out v))
                {
                    // Legacy MS format: "5.8.9586.32322"
                    // These are ALWAYS considered older than the new timestamp system
                    // because the timestamp system starts from year 26 (260101....)
                    // while MS build day 9586 is only 11 digits total in our logic
                    localNum = long.Parse(
                        string.Format("{0:D3}{1:D3}{2}{3}", v.Major, v.Minor, v.Build, v.Revision)
                    );
                }
                else
                {
                    localNum = long.Parse(localDigitsOnly);
                }

                // 3. Comparison
                return remoteNum > localNum;
            }
            catch
            {
                // If parsing fails, we assume it's a very old version and needs update
                return true;
            }
        }
    }
}
