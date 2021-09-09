using System.Collections.Generic;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// LoginPage.xaml 的交互逻辑
    /// </summary>
    public partial class LoginPage : Page
    {
        public List<string> item_TW = new List<string>
        {
            "帳號密碼",
            "QR Code便利登"
        };
        public List<string> item_HK = new List<string>
        {
            "帳號密碼"
        };
        public id_pass_form id_pass = new id_pass_form();
        public qr_form qr = new qr_form();

        public LoginPage()
        {
            InitializeComponent();
            
            if (ConfigAppSettings.GetValue("loginRegion", "TW") == "TW")
                Beanfun_TW.IsEnabled = false;
            else
                Beanfun_TW.IsEnabled = true;
            Beanfun_HK.IsEnabled = !Beanfun_TW.IsEnabled;
        }

        private void Beanfun_TW_Click(object sender, RoutedEventArgs e)
        {
            if (!Beanfun_TW.IsEnabled)
                return;
            Beanfun_TW.IsEnabled = !Beanfun_TW.IsEnabled;
            Beanfun_HK.IsEnabled = !Beanfun_TW.IsEnabled;
            App.LoginRegion = "TW";
            ConfigAppSettings.SetValue("loginRegion", App.LoginRegion);
            App.MainWnd.ddlAuthTypeItemsInit();
            App.MainWnd.reLoadGameInfo();
        }

        private void Beanfun_HK_Click(object sender, RoutedEventArgs e)
        {
            if (!Beanfun_HK.IsEnabled)
                return;
            Beanfun_HK.IsEnabled = !Beanfun_HK.IsEnabled;
            Beanfun_TW.IsEnabled = !Beanfun_HK.IsEnabled;
            App.LoginRegion = "HK";
            ConfigAppSettings.SetValue("loginRegion", App.LoginRegion);
            App.MainWnd.ddlAuthTypeItemsInit();
            App.MainWnd.reLoadGameInfo();
        }

        private void Label_MouseDoubleClick(object sender, MouseButtonEventArgs e)
        {
            new AccRecovery(App.MainWnd.accountManager).ShowDialog();
        }
    }
}
