using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace Beanfun
{
    /// <summary>
    /// Settings.xaml 的交互逻辑
    /// </summary>
    public partial class Settings : Page
    {
        public Settings()
        {
            InitializeComponent();

            autoStartGame.IsChecked = bool.Parse(ConfigAppSettings.GetValue("autoStartGame", "false"));
            ask_update.IsChecked = bool.Parse(ConfigAppSettings.GetValue("ask_update", "true"));
            minimize_to_tray.IsChecked = bool.Parse(ConfigAppSettings.GetValue("minimize_to_tray", "false"));

            tradLogin.IsChecked = bool.Parse(ConfigAppSettings.GetValue("tradLogin", "true"));
            skipPlayWnd.IsChecked = bool.Parse(ConfigAppSettings.GetValue("skipPlayWnd", "true"));
            autoKillPatcher.IsChecked = bool.Parse(ConfigAppSettings.GetValue("autoKillPatcher", "true"));

            cb_UpdateChannel.SelectedIndex = ConfigAppSettings.GetValue("updateChannel", "Stable").Equals("Stable") ? 0 : 1;

            cb_ThemeColor.Text = ConfigAppSettings.GetValue("ThemeColor", "#B6DE8E");
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.frame == null) return;
            if (App.MainWnd.return_page == null) App.MainWnd.return_page = App.MainWnd.loginPage;
            App.MainWnd.frame.Content = App.MainWnd.return_page;
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
                if (App.MainWnd.win_class_name != null && App.MainWnd.win_class_name != "" && App.MainWnd.game_exe != "" && App.MainWnd.login_action_type != 1)
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
                Color color = (Color)ColorConverter.ConvertFromString(cb_ThemeColor.Text);
                App.MainWnd.changeThemeColor(color);
                ConfigAppSettings.SetValue("ThemeColor", cb_ThemeColor.Text);
            } catch { }
        }
    }
}
