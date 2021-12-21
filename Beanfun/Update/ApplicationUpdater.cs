using Amemiya.Net;
using System;
using System.Diagnostics;
using System.Windows;
using System.Xml;

namespace Beanfun.Update
{
    class ApplicationUpdater
    {
        private static string baseUrl = $"https://ghproxy.com/https://raw.githubusercontent.com/pungin/Beanfun/{ (ConfigAppSettings.GetValue("updateChannel", "Stable").Equals("Stable") ? "master" : "beta") }/";

        internal static void CheckApplicationUpdate(bool show)
        {
            var url = $"{baseUrl}VersionInfo.xml";

            try
            {
                var client = new WebClientEx(10 * 1000);
                var stream = client.DownloadDataStream(url);

                var xmlContent = new XmlDocument();
                xmlContent.Load(stream);

                ProcessUpdate(xmlContent, System.Reflection.Assembly.GetExecutingAssembly().GetName().Version, show);
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

                    MessageBoxResult result = MessageBox.Show($"檢測到新版本 {App.ConvertVersion(version)} 當前: {App.ConvertVersion(crtVer)}\r\n\r\n{note}\r\n" + "\r\n" + "是否打開下載頁面？", "更新檢測", MessageBoxButton.OKCancel);
                    if (result == MessageBoxResult.OK) Process.Start("https://github.com/pungin/Beanfun/releases");
                }
                catch (Exception) { }
            }
            else
            {
                if (show) MessageBox.Show($"未檢測到有更新。", "更新檢測", MessageBoxButton.OK);
            }
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
