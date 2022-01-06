using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Windows;
using System.Windows.Markup;

namespace Beanfun
{
    class I18n
    {
        public static string CultureName { get; set; }
        public static List<string> CultureArray { get; } = new List<string>();

        internal static void LoadLanguage(string lang = null)
        {
            CultureInfo currentCultureInfo;
            if (lang == null)
                currentCultureInfo = CultureInfo.CurrentUICulture;
            else
            {
                currentCultureInfo = CultureInfo.GetCultureInfo(lang);
                if (currentCultureInfo == null) currentCultureInfo = CultureInfo.CurrentUICulture;
            }

            CultureName = currentCultureInfo.Name;
            CultureArray.Clear();
            while (true)
            {
                if (string.IsNullOrEmpty(currentCultureInfo.Name))
                    break;
                CultureArray.Insert(0, currentCultureInfo.Name);
                currentCultureInfo = currentCultureInfo.Parent;
            }

            ResourceDictionary defaultDict = null;
            if (Application.Current.Resources.MergedDictionaries.Count > 0) defaultDict = Application.Current.Resources.MergedDictionaries[0];
            Application.Current.Resources.MergedDictionaries.Clear();
            if (defaultDict != null) Application.Current.Resources.MergedDictionaries.Add(defaultDict);

            try
            {
                var langDir = Path.Combine(Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location), @"Lang\");

                ResourceDictionary dictionary = null;
                string langPath = null;
                string langUri = null;
                foreach (string cultureName in CultureArray)
                {
                    langPath = Path.Combine(langDir, cultureName + ".xaml");
                    if (File.Exists(langPath))
                        dictionary = XamlReader.Load(new FileStream(langPath, FileMode.Open)) as ResourceDictionary;
                    else if (!cultureName.ToUpper().Equals("ZH"))
                    {
                        langUri = $@"/Beanfun;Component/Lang/{ cultureName }.xaml";
                        try
                        {
                            dictionary = new ResourceDictionary
                            {
                                Source = new Uri(langUri, UriKind.Relative)
                            };
                        }
                        catch { }
                    }

                    if (dictionary != null) Application.Current.Resources.MergedDictionaries.Add(dictionary);
                }
            }
            catch { }

            if (Application.Current.Resources.MergedDictionaries.Count <= 0) throw new Exception("No language file.");
        }

        internal static string ToSimplified(string argSource)
        {
            if (!CultureArray.Contains("zh-Hans")) return argSource;
            var t = new String(' ', argSource.Length);
            WindowsAPI.LCMapStringW(CultureInfo.CurrentUICulture.LCID, (int)WindowsAPI.dwMapFlags.LCMAP_SIMPLIFIED_CHINESE, argSource, argSource.Length, t, argSource.Length);
            return t;
        }
    }
}
