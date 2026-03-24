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
            var url = $"https://api.github.com/repos/pungin/beanfun/releases";

            try
            {
                var client = new WebClient();
                client.Headers.Add("User-Agent", $"Beanfun(V{App.AssemblyVersion})");
                client.Headers.Add("Accept", "application/vnd.github.v3+json");
                var json = Encoding.UTF8.GetString(client.DownloadData(url));

                var releases = JsonConvert.DeserializeObject<List<GitHubRelease>>(json);
                GitHubRelease release = GetLastRelease(releases);
                if (release == null)
                    return;

                var match = Regex.Match(release.TagName, @"^v(\d+)\.(\d+)\.(\d+)$");
                if (!match.Success)
                    return;
                string newVer =
                    $"{match.Groups[1].Value}.{match.Groups[2].Value}({match.Groups[3].Value})";
                if (
                    CompareVersion(
                        System.Reflection.Assembly.GetExecutingAssembly().GetName().Version,
                        App.ParseVersion(newVer)
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
                                newVer,
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
                if (!stable && release.Prerelease)
                    return release;
                else if (!release.Prerelease)
                    return release;
            }
            return null;
        }

        /// <summary>
        ///     If newVer is bigger than oldVer, return true.
        /// </summary>
        /// <param name="oldVer"></param>
        /// <param name="newVer"></param>
        /// <returns></returns>
        private static bool CompareVersion(Version oldVer, Version newVer)
        {
            return oldVer < newVer;
        }
    }
}
