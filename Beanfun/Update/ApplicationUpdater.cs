using Amemiya.Net;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Xml;

namespace Beanfun.Update
{
    class ApplicationUpdater
    {
        private static string baseUrl = $"https://raw.githubusercontent.com/pungin/Beanfun/{ (ConfigAppSettings.GetValue("updateChannel", "Stable").Equals("Stable") ? "master" : "beta") }/";
        private static DownloadProgressBar downloadProgressBar;
        private static List<string> taskFiles = new List<string>();

        internal static void CheckApplicationUpdate(Version version, bool show)
        {
            var url = $"{baseUrl}VersionInfo.xml";

            try
            {
                var client = new WebClientEx(10 * 1000);
                var stream = client.DownloadDataStream(url);

                var xmlContent = new XmlDocument();
                xmlContent.Load(stream);

                ProcessUpdate(xmlContent, version, show);
            }
            catch (Exception) { }
        }

        private static void ProcessUpdate(XmlDocument xmlContent, Version crtVer, bool show)
        {
            var newVer = xmlContent.SelectSingleNode(@"/VersionInfo/Version/text()").Value;

            if (CompareVersion(crtVer, new Version(newVer)))
            {
                try
                {
                    Version version = new Version(xmlContent.SelectSingleNode(@"/VersionInfo/Version/text()").Value);
                    var date = xmlContent.SelectSingleNode(@"/VersionInfo/Date/text()").Value;
                    var note = xmlContent.SelectSingleNode(@"/VersionInfo/Note/text()").Value;

                    MessageBoxResult result = System.Windows.MessageBox.Show($"檢測到新版本 {version.Major}.{version.Minor}.{version.Build}({version.Revision}) 當前: {crtVer.Major}.{crtVer.Minor}.{crtVer.Build}({crtVer.Revision})\r\n\r\n{note}\r\n" + "\r\n" + "是否更新(會重啟軟體)？", "更新檢測", MessageBoxButton.OKCancel);
                    if (result == MessageBoxResult.OK)
                    {
                        StartUpdate(xmlContent);
                    }
                }
                catch (Exception) { }
            }
            else
            {
                if (show)
                {
                    MessageBoxResult result = System.Windows.MessageBox.Show($"未檢測到有更新。", "更新檢測", MessageBoxButton.OK);
                }
            }
        }

        private static void StartUpdate(XmlDocument xmlContent)
        {
            string url = xmlContent.SelectSingleNode(@"/VersionInfo/Url/text()").Value;
            Version updaterVersion = new Version(xmlContent.SelectSingleNode(@"/VersionInfo/UpdaterVersion/text()").Value);
            
            string baseDir = $"{System.Environment.CurrentDirectory}\\";
            taskFiles.Clear();
            if (!File.Exists($"{baseDir}BFUpdater.exe") || CompareVersion(GetFileVersion($"{baseDir}BFUpdater.exe"), updaterVersion))
            {
                DownloadFile(taskFiles, $"{baseUrl}BFUpdater.exe", baseDir);
            }
            DownloadFile(taskFiles, url, baseDir);
            downloadProgressBar = new DownloadProgressBar(taskFiles, "正在下載更新...", baseDir, true);
            downloadProgressBar.Closing += DownloadProgressBar_Closing;
            downloadProgressBar.ShowDialog();
        }

        private static void DownloadProgressBar_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            if (downloadProgressBar.TaskFileNum > 0 && downloadProgressBar.TaskFileNum == downloadProgressBar.DownloadedFileNum)
            {
                Process.Start($"{System.Environment.CurrentDirectory}\\BFUpdater.exe");
            }
            else
            {
                string baseDir = $"{System.Environment.CurrentDirectory}\\";
                foreach (string uri in taskFiles)
                {
                    string fileName = uri.Substring(uri.LastIndexOf("/") + 1);
                    string path = $"{baseDir}{fileName}";
                    if (File.Exists(path))
                    {
                        File.Delete(path);
                    }
                }
            }
        }

        private static void DownloadFile(List<string> taskFiles, string url, string path)
        {
            path = $"{ path }{ url.Substring(url.LastIndexOf("/") + 1) }";
            if (File.Exists(path))
            {
                File.Delete(path);
            }
            taskFiles.Add(url);
        }

        private static Version GetFileVersion(string path)
        {
            string version = null;
            try {
                version = FileVersionInfo.GetVersionInfo(path).FileVersion;
            } catch {}
            return new Version(version == null ? "0.0.0.0" : version);
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
