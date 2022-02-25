using System;
using System.Collections.Generic;
using System.Windows;
using System.Windows.Controls;

namespace Beanfun
{
    /// <summary>
    /// Settings.xaml 的交互逻辑
    /// </summary>
    public partial class Settings : Page
    {
        class Language
        {
            public string Name {get; set; }
            public string DisplayName { get; set; }
        }

        public Settings()
        {
            InitializeComponent();

            List<Language> languageList = new List<Language>();
            languageList.Add(new Language { Name = "zh-Hant", DisplayName = "中文(繁體)" });
            languageList.Add(new Language { Name = "zh-Hans", DisplayName = "中文(简体)" });
            cb_Language.ItemsSource = languageList;
            cb_Language.DisplayMemberPath = "DisplayName";
            cb_Language.SelectedValuePath = "Name";
            string cultureName = I18n.CultureName.ToUpper();
            string name = null;
            foreach (Language language in languageList)
            {
                if (language.Name.ToUpper().Equals(cultureName))
                {
                    name = language.Name;
                    break;
                }
            }
            if (name == null)
            {
                name = "zh-Hant";
                switch (cultureName)
                {
                    case "ZH-CHS":
                    case "ZH-CN":
                    case "ZH-SG":
                    case "ZH-MY":
                    case "ZH-HANS-HK":
                    case "ZH-HANS-MO":
                        name = "zh-Hans";
                        break;
                }
            }
            cb_Language.SelectedValue = name;

            autoStartGame.IsChecked = bool.Parse(ConfigAppSettings.GetValue("autoStartGame", "false"));
            ask_update.IsChecked = bool.Parse(ConfigAppSettings.GetValue("ask_update", "true"));
            minimize_to_tray.IsChecked = bool.Parse(ConfigAppSettings.GetValue("minimize_to_tray", "false"));

            tradLogin.IsChecked = bool.Parse(ConfigAppSettings.GetValue("tradLogin", "true"));
            skipPlayWnd.IsChecked = bool.Parse(ConfigAppSettings.GetValue("skipPlayWnd", "true"));
            autoKillPatcher.IsChecked = bool.Parse(ConfigAppSettings.GetValue("autoKillPatcher", "true"));

            cb_UpdateChannel.SelectedIndex = ConfigAppSettings.GetValue("updateChannel", "Stable").Equals("Stable") ? 0 : 1;

            cb_ThemeColor.Text = ConfigAppSettings.GetValue("ThemeColor", "#FF8201");
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.frame == null) return;
            if (App.MainWnd.return_page == null || App.MainWnd.return_page == App.MainWnd.loginPage) App.MainWnd.NavigateLoginPage();
            else App.MainWnd.frame.Content = App.MainWnd.return_page;
            App.MainWnd.return_page = null;
        }

        private void skipPlayWnd_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.settingPage == null || App.MainWnd.checkPlayPage == null || skipPlayWnd.IsChecked == bool.Parse(ConfigAppSettings.GetValue("skipPlayWnd", "true")))
                return;
            ConfigAppSettings.SetValue("skipPlayWnd", Convert.ToString((bool)skipPlayWnd.IsChecked));
            App.MainWnd.checkPlayPage.IsEnabled = (bool)skipPlayWnd.IsChecked;
        }

        private void autoKillPatcher_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.settingPage == null || autoKillPatcher.IsChecked == bool.Parse(ConfigAppSettings.GetValue("autoKillPatcher", "true")))
                return;
            ConfigAppSettings.SetValue("autoKillPatcher", Convert.ToString((bool)autoKillPatcher.IsChecked));
            App.MainWnd.checkPatcher.IsEnabled = (bool)autoKillPatcher.IsChecked;
        }

        private void autoStartGame_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.settingPage == null || autoStartGame.IsChecked == bool.Parse(ConfigAppSettings.GetValue("autoStartGame", "false")))
                return;
            ConfigAppSettings.SetValue("autoStartGame", Convert.ToString((bool)autoStartGame.IsChecked));
        }

        private void ask_update_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.settingPage == null || ask_update.IsChecked == bool.Parse(ConfigAppSettings.GetValue("ask_update", "true")))
                return;
            ConfigAppSettings.SetValue("ask_update", Convert.ToString((bool)ask_update.IsChecked));
        }

        private void tradLogin_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.accountList == null || App.MainWnd.accountList.panel_GetOtp == null || App.MainWnd.accountList.autoPaste == null)
                return;
            if ((bool)tradLogin.IsChecked)
            {
                App.MainWnd.accountList.panel_GetOtp.Visibility = Visibility.Visible;

                if ("MapleStoryClass".Equals(App.MainWnd.win_class_name))
                    App.MainWnd.accountList.autoPaste.Visibility = Visibility.Visible;
                else
                    App.MainWnd.accountList.autoPaste.Visibility = Visibility.Collapsed;
            }
            else
            {
                App.MainWnd.accountList.panel_GetOtp.Visibility = Visibility.Collapsed;
            }
            if (App.MainWnd.settingPage == null || bool.Parse(ConfigAppSettings.GetValue("tradLogin", "true")) == tradLogin.IsChecked)
                return;
            ConfigAppSettings.SetValue("tradLogin", Convert.ToString(tradLogin.IsChecked));
        }

        private void minimize_to_tray_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.settingPage == null || minimize_to_tray.IsChecked == bool.Parse(ConfigAppSettings.GetValue("minimize_to_tray", "false")))
                return;
            ConfigAppSettings.SetValue("minimize_to_tray", Convert.ToString((bool)minimize_to_tray.IsChecked));
        }

        private void cb_UpdateChannel_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.settingPage == null || cb_UpdateChannel.SelectedIndex == (ConfigAppSettings.GetValue("updateChannel", "Stable").Equals("Stable") ? 0 : 1))
                return;
            ConfigAppSettings.SetValue("updateChannel", cb_UpdateChannel.SelectedIndex == 0 ? "Stable" : "Beta");
        }

        private void cb_ThemeColor_TextChanged(object sender, System.EventArgs e)
        {
            try
            {
                App.MainWnd.changeThemeColor(cb_ThemeColor.Text);
                ConfigAppSettings.SetValue("ThemeColor", cb_ThemeColor.Text);
            } catch { }
        }

        private void ManageAcc_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.frame.Content = App.MainWnd.manageAccPage;
        }

        private void btn_Tools_Click(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd.accountList != null) App.MainWnd.accountList.btn_Tools_Click(null, null);
        }

        private void cb_Language_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            string language = ConfigAppSettings.GetValue("Language", null);
            if (App.MainWnd == null || App.MainWnd.settingPage == null || (language != null && cb_Language.SelectedValue.ToString().ToUpper().Equals(language.ToUpper())))
                return;
            language = cb_Language.SelectedValue.ToString();
            ConfigAppSettings.SetValue("Language", language);
            I18n.LoadLanguage(language);
        }
    }
}
