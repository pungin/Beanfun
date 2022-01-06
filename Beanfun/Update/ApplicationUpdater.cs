using Amemiya.Net;
using System;
using System.Diagnostics;
using System.Windows;
using System.Xml;

namespace Beanfun.Update
{
    class ApplicationUpdater
    {
        private static string baseUrl = $"https://raw.githubusercontent.com/pungin/Beanfun/{ (ConfigAppSettings.GetValue("updateChannel", "Stable").Equals("Stable") ? "master" : "beta") }/";

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

                    MessageBoxResult result = MessageBox.Show(string.Format((Application.Current.TryFindResource("NewVersionDetected") as string).Replace("\\r\\n", "\r\n"),
                        App.ConvertVersion(version), App.ConvertVersion(crtVer), note),
                        Application.Current.TryFindResource("UpdateCheck") as string, MessageBoxButton.OKCancel);
                    if (result == MessageBoxResult.OK) Process.Start("https://github.com/pungin/Beanfun/releases");
                }
                catch (Exception) { }
            }
            else
            {
                if (show) MessageBox.Show(Application.Current.TryFindResource("NoUpdatesDetected") as string,
                    Application.Current.TryFindResource("UpdateCheck") as string, MessageBoxButton.OK);
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
