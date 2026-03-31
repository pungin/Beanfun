using System;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;

namespace Beanfun
{
    /// <summary>
    /// App.xaml 的交互逻辑
    /// </summary>
    public partial class App : Application
    {
        public static readonly Version OSVersion = Environment.OSVersion.Version;
        public static readonly Version Win2000 = new Version(5, 0);
        public static readonly Version WinXP = new Version(5, 1);
        public static readonly Version Win2003 = new Version(5, 2);
        public static readonly Version WinVista = new Version(6, 0);
        public static readonly Version Win7 = new Version(6, 1);
        public static readonly Version Win8 = new Version(6, 2);
        public static readonly Version Win8_1 = new Version(6, 3);
        public static readonly Version Win10 = new Version(10, 0);
        public static readonly Version Win11 = new Version(10, 0, 22000, 0);

        public static MainWindow MainWnd
        {
            get
            {
                Window wnd = Current.MainWindow;
                if (wnd != null && (typeof(MainWindow) == wnd.GetType()))
                    return (MainWindow)wnd;
                else
                    return null;
            }
        }

        public static string LoginRegion = ConfigAppSettings.GetValue("loginRegion", "TW");
        public static int LoginMethod = int.Parse(ConfigAppSettings.GetValue("loginMethod", "0"));

        private void Main(object sender, StartupEventArgs e)
        {
            WindowsAPI.AttachConsole(-1);

            if (bool.Parse(ConfigAppSettings.GetValue("disableHardwareAcceleration", "false")))
                RenderOptions.ProcessRenderMode = RenderMode.SoftwareOnly;

            I18n.LoadLanguage(ConfigAppSettings.GetValue("Language", null));

            StartupUri = new Uri("MainWindow.xaml", UriKind.RelativeOrAbsolute);
        }

        public bool compareFile(string path1, string path2)
        {
            using var hash = MD5.Create();
            using var stream_1 = File.OpenRead(path1);
            byte[] hashByte_1 = hash.ComputeHash(stream_1);

            using var stream_2 = File.OpenRead(path2);
            byte[] hashByte_2 = hash.ComputeHash(stream_2);

            return BitConverter.ToString(hashByte_1) == BitConverter.ToString(hashByte_2);
        }

        private void Application_Exit(object sender, ExitEventArgs e)
        {
            if (MainWnd != null && MainWnd.bfClient != null)
                try
                {
                    MainWnd.bfClient.Logout();
                }
                catch { }
        }

        // --- 版本解析邏輯 (支援 3 段及 4 段格式) ---
        public static Version ParseVersion(string version)
        {
            // 1. 支援新式四段 (v5.8.3(2603311841))
            var match4 = Regex.Match(version, @"^v?(\d+)\.(\d+)\.(\d+)\((\d{10})\)$");
            if (match4.Success)
            {
                var buildDate = DateTime.ParseExact(
                    match4.Groups[4].Value,
                    "yyMMddHHmm",
                    CultureInfo.InvariantCulture
                );
                return new Version(
                    int.Parse(match4.Groups[1].Value),
                    int.Parse(match4.Groups[2].Value),
                    int.Parse(match4.Groups[3].Value), // Patch Number
                    (int)(buildDate.TimeOfDay.TotalSeconds / 2) // Revision
                );
            }

            // 2. 支援新式三段 (v5.8(2603311841))
            var match3 = Regex.Match(version, @"^v?(\d+)\.(\d+)\((\d{10})\)$");
            if (match3.Success)
            {
                var buildDate = DateTime.ParseExact(
                    match3.Groups[3].Value,
                    "yyMMddHHmm",
                    CultureInfo.InvariantCulture
                );
                var baseDate = new DateTime(2000, 1, 1);
                return new Version(
                    int.Parse(match3.Groups[1].Value),
                    int.Parse(match3.Groups[2].Value),
                    (int)(buildDate - baseDate).TotalDays,
                    (int)(buildDate.TimeOfDay.TotalSeconds / 2)
                );
            }

            // 3. 支援舊式 (5.8.9586(33854))
            var oldMatch = Regex.Match(version, @"^(\d+)\.(\d+)\.(\d+)\((\d+)\)$");
            if (oldMatch.Success)
            {
                return new Version(
                    int.Parse(oldMatch.Groups[1].Value),
                    int.Parse(oldMatch.Groups[2].Value),
                    int.Parse(oldMatch.Groups[3].Value),
                    int.Parse(oldMatch.Groups[4].Value)
                );
            }

            throw new FormatException("Invalid version format: " + version);
        }

        // --- 版本轉換邏輯 (處理幽靈點問題) ---
        public static string ConvertVersion(Version version)
        {
            if (version < new Version(4, 1))
                return $"{version.Major}.{version.Minor}.{version.Build}({version.Revision})";

            DateTime buildDate = new DateTime(2000, 1, 1)
                .AddDays(version.Build)
                .AddSeconds(version.Revision * 2);

            string timestamp = buildDate.ToString("yyMMddHHmm");

            // 關鍵：如果 Build < 1000 代表係 Patch 號碼
            if (version.Build < 1000)
            {
                // 格式: 5.8.3(2604011114)
                return $"{version.Major}.{version.Minor}.{version.Build}({timestamp})";
            }
            else
            {
                // 格式: 5.8(2604011114) -> 絕對唔加多餘嘅點
                return $"{version.Major}.{version.Minor}({timestamp})";
            }
        }

        internal static string AssemblyVersion
        {
            get { return ConvertVersion(Assembly.GetExecutingAssembly().GetName().Version); }
        }

        public static int ReleaseResource(string file)
        {
            string baseDir = Path.GetDirectoryName(
                System.Diagnostics.Process.GetCurrentProcess().MainModule.FileName
            );
            string path = Path.Combine(baseDir, file);
            using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream(file))
            {
                if (stream != null)
                {
                    if (File.Exists(path))
                    {
                        var fileInfo = new FileInfo(path);
                        if (fileInfo.Length == stream.Length)
                            return 0;

                        try
                        {
                            File.Delete(path);
                        }
                        catch
                        {
                            return -1;
                        }
                    }

                    string dir = Path.GetDirectoryName(path);
                    if (!Directory.Exists(dir))
                        Directory.CreateDirectory(dir);

                    stream.Position = 0;
                    File.WriteAllBytes(
                        path,
                        new BinaryReader(stream).ReadBytes((int)stream.Length)
                    );
                    return 1;
                }
            }
            return -1;
        }

        public static string GetMD5HashFromFile(string fileName)
        {
            try
            {
                using FileStream file = new FileStream(
                    fileName,
                    FileMode.Open,
                    FileAccess.Read,
                    FileShare.ReadWrite
                );
                return GetMD5HashFromStream(file);
            }
            catch (Exception ex)
            {
                throw new Exception("GetMD5HashFromFile() fail, error: " + ex.Message);
            }
        }

        public static string GetMD5HashFromStream(Stream stream)
        {
            try
            {
                using MD5 md5 = MD5.Create();
                byte[] retVal = md5.ComputeHash(stream);
                StringBuilder sb = new StringBuilder();
                for (int i = 0; i < retVal.Length; i++)
                {
                    sb.Append(retVal[i].ToString("x2"));
                }
                return sb.ToString();
            }
            catch (Exception ex)
            {
                throw new Exception("GetMD5HashFromStream() fail, error: " + ex.Message);
            }
        }
    }
}
