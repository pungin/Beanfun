using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Windows.Forms;

namespace BFUpdater
{
    static class Program
    {
        /// <summary>
        /// 应用程序的主入口点。
        /// </summary>
        [STAThread]
        static void Main()
        {
            //Application.EnableVisualStyles();
            //Application.SetCompatibleTextRenderingDefault(false);

            string baseDir = $"{System.Environment.CurrentDirectory}\\";
            string tempDir = $"{Path.GetTempPath()}UpdateTmp";
            if (File.Exists($"{baseDir}Beanfun.zip"))
            {
                Process[] processes = Process.GetProcessesByName("Beanfun");
                foreach (Process process in processes)
                {
                    try {
                        string processPath = process.MainModule.FileName.ToLower().Replace("/", "\\");
                        if (string.Compare(processPath, $"{baseDir}Beanfun.exe".ToLower().Replace("/", "\\"), true) == 0)
                        {
                            process.Kill();
                        }
                    } catch {}
                }

                string lastGameBinName = ConfigAppSettings.GetValue("LastGameBinName", "");
                if (lastGameBinName != "")
                {
                    processes = Process.GetProcessesByName(lastGameBinName);
                    foreach (Process process in processes)
                    {
                        try
                        {
                            string processPath = process.MainModule.FileName.ToLower().Replace("/", "\\");
                            if (string.Compare(processPath, $"{baseDir}{lastGameBinName}.exe".ToLower().Replace("/", "\\"), true) == 0)
                            {
                                process.Kill();
                            }
                        }
                        catch { }
                    }
                    if (deleteFile($"{baseDir}{lastGameBinName}.exe"))
                    {
                        ConfigAppSettings.SetValue("LastGameBinName", null);
                    }
                }

                string oldVersion = GetFileVersion($"{baseDir}Beanfun.exe");

                deleteFile($"{baseDir}LoaderDll.dll");
                deleteFile($"{baseDir}LocaleEmulator.dll");

                string checkVersion;

                checkVersion = "1.3.4.1";
                if (CompareVersion(oldVersion, checkVersion))
                {
                    deleteFile($"{baseDir}FluentWPF.dll");
                    deleteFile($"{baseDir}Interop.BFService.dll");
                    deleteFile($"{baseDir}Interop.FSFISCATLLib.dll");
                    deleteFile($"{baseDir}Interop.FSP11CRYPTATLLib.dll");
                    deleteFile($"{baseDir}Newtonsoft.Json.dll");
                }

                checkVersion = "2.2.0.1";
                if (CompareVersion(oldVersion, checkVersion))
                {
                    deleteFile(string.Format("{0}init.ini", baseDir));
                    if (!Directory.Exists(System.Environment.GetFolderPath(System.Environment.SpecialFolder.ApplicationData)))
                    {
                        Directory.CreateDirectory(System.Environment.GetFolderPath(System.Environment.SpecialFolder.ApplicationData));
                    }
                    string target = System.Environment.GetFolderPath(System.Environment.SpecialFolder.ApplicationData) + "\\";
                    moveFile(baseDir, target, "Config.xml");
                    moveFile(baseDir, target, "Users.dat");
                    if (Directory.Exists($"{baseDir}Plugins"))
                    {
                        try
                        {
                            Directory.Delete($"{baseDir}Plugins", true);
                        } catch { }
                    }
                }

                checkVersion = "2.4.3.1";
                if (CompareVersion(oldVersion, checkVersion))
                {
                    processes = Process.GetProcessesByName("MapleStory");
                    foreach (Process process in processes)
                    {
                        try
                        {
                            string processPath = process.MainModule.FileName.ToLower().Replace("/", "\\");
                            if (string.Compare(processPath, $"{baseDir}MapleStory.exe".ToLower().Replace("/", "\\"), true) == 0)
                            {
                                process.Kill();
                            }
                        }
                        catch { }
                    }
                    deleteFile($"{baseDir}MapleStory.exe");

                    ConfigAppSettings.SetValue("ask_create_shortcut", null);
                }

                checkVersion = "2.9.5.0";
                if (CompareVersion(oldVersion, checkVersion))
                {
                    ConfigAppSettings.SetValue("compatibility_mode", null);
                    ConfigAppSettings.SetValue("LastGameBinName", null);
                }

                checkVersion = "3.1.1.3";
                if (CompareVersion(oldVersion, checkVersion))
                {
                    deleteFile($"{baseDir}Fonts");
                    deleteFile($"{baseDir}FluentWPF.dll");
                    deleteFile($"{baseDir}FluorineFx.dll");
                    deleteFile($"{baseDir}INIFileParser.dll");
                    deleteFile($"{baseDir}Interop.BFService.dll");
                    deleteFile($"{baseDir}log4net.dll");
                    deleteFile($"{baseDir}Newtonsoft.Json.dll");
                }

                deleteFile(tempDir);
                if (Directory.Exists(tempDir))
                {
                    Directory.Delete(tempDir, true);
                }

                ZipFile.ExtractToDirectory($"{baseDir}Beanfun.zip", tempDir);

                List<string> dirs = new List<string>(Directory.GetDirectories(tempDir));
                dirs.ForEach(c =>
                {
                    string destFile = Path.Combine(new string[] { baseDir, Path.GetFileName(c) });
                    deleteFile(destFile);
                    Directory.Move(c, destFile);
                });
                List<string> files = new List<string>(Directory.GetFiles(tempDir));
                files.ForEach(c =>
                {
                    string destFile = Path.Combine(new string[] { baseDir, Path.GetFileName(c) });
                    deleteFile(destFile);
                    File.Move(c, destFile);
                });

                List<string> folders = new List<string>(Directory.GetDirectories(tempDir));
                folders.ForEach(c =>
                {
                    string destDir = Path.Combine(new string[] { baseDir, Path.GetFileName(c) });
                    if (Directory.Exists(destDir))
                    {
                        Directory.Delete(destDir, true);
                    }
                    Directory.Move(c, destDir);
                });

                if (Directory.Exists(tempDir))
                {
                    Directory.Delete(tempDir, true);
                }

                deleteFile($"{baseDir}Beanfun.zip");

                Process.Start($"{baseDir}Beanfun.exe");
            }
        }

        private static bool deleteFile(string dir)
        {
            if (File.Exists(dir))
            {
                try
                {
                    File.Delete(dir);
                } catch { return false; }
            }
            else if (Directory.Exists(dir))
            {
                try
                {
                    Directory.Delete(dir, true);
                } catch { return false; }
            }
            return true;
        }

        private static void moveFile(string src, string target, string name)
        {
            if (File.Exists(src))
            {
                try
                {
                    File.Move(src + "name", target + "name");
                } catch { }
            }
        }

        private static string GetFileVersion(string path)
        {
            string version = null;
            try
            {
                version = FileVersionInfo.GetVersionInfo(path).FileVersion;
            }
            catch { }
            return version == null ? "0.0.0.0" : version;
        }

        private static bool CompareVersion(string oldVer, string newVer)
        {
            var versionOld = new Version(oldVer);
            var versionNew = new Version(newVer);

            return versionOld <= versionNew;
        }
    }
}