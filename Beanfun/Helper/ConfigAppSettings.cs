using System.Configuration;
using System.IO;

namespace Beanfun
{
    class ConfigAppSettings
    {
        public static void SetValue(string key, string value)
        {
            try
            {
                ExeConfigurationFileMap map = new ExeConfigurationFileMap();
                map.ExeConfigFilename = System.Environment.GetFolderPath(System.Environment.SpecialFolder.ApplicationData) + "\\Beanfun\\Config.xml";
                Configuration config = ConfigurationManager.OpenMappedExeConfiguration(map, ConfigurationUserLevel.None);
                if (config.AppSettings.Settings[key] == null)
                {
                    if (value != null)
                        config.AppSettings.Settings.Add(key, value);
                }
                else
                {
                    if (value == null)
                        config.AppSettings.Settings.Remove(key);
                    else
                        config.AppSettings.Settings[key].Value = value;
                }
                config.Save(ConfigurationSaveMode.Modified);
                ConfigurationManager.RefreshSection("appSettings");
            }
            catch
            {
                try
                {
                    string filePath = System.Environment.GetFolderPath(System.Environment.SpecialFolder.ApplicationData) + "\\Beanfun";
                    DirectoryInfo dir = new DirectoryInfo(filePath);
                    FileSystemInfo[] fileinfo = dir.GetFileSystemInfos("Config.xml");
                    foreach (FileSystemInfo i in fileinfo)
                    {
                        if (i is DirectoryInfo)
                        {
                            DirectoryInfo subdir = new DirectoryInfo(i.FullName);
                            subdir.Delete(true);
                        }
                        else
                        {
                            File.Delete(i.FullName);
                        }
                    }
                    SetValue(key, value);
                } catch { }
            }
        }

        public static string GetValue(string key)
        {
            return GetValue(key, string.Empty);
        }

        public static string GetValue(string key, string def)
        {
            string value;
            try
            {
                ExeConfigurationFileMap map = new ExeConfigurationFileMap();
                map.ExeConfigFilename = System.Environment.GetFolderPath(System.Environment.SpecialFolder.ApplicationData) + "\\Beanfun\\Config.xml";
                Configuration config = ConfigurationManager.OpenMappedExeConfiguration(map, ConfigurationUserLevel.None);
                value = config.AppSettings.Settings[key] == null ? def : config.AppSettings.Settings[key].Value;
            }
            catch
            {
                value = def;
            }
            return value;
        }
    }
}
