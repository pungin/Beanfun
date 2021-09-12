using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// AccountList.xaml 的交互逻辑
    /// </summary>
    public partial class AccountList : Page
    {
        public AccountList()
        {
            InitializeComponent();

            autoPaste.IsChecked = bool.Parse(ConfigAppSettings.GetValue("autoPaste", "false"));
        }

        private void btn_Logout_Click(object sender, RoutedEventArgs e)
        {
            MessageBoxResult result = MessageBox.Show("即將登出，是否要繼續？", "登出", MessageBoxButton.YesNo);
            if (result == MessageBoxResult.No) return;
            if (App.LoginMethod == (int)LoginMethod.QRCode) App.MainWnd.loginMethodChanged();
            App.MainWnd.NavigateLoginPage();
        }

        private void list_Account_MouseDoubleClick(object sender, RoutedEventArgs e)
        {
            BeanfunClient.ServiceAccount account = (BeanfunClient.ServiceAccount)list_Account.SelectedItem;
            if (account == null)
                return;
            try
            {
                Clipboard.SetText(account.sid);
            }
            catch { }
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (((bool)App.MainWnd.settingPage.tradLogin.IsChecked && App.MainWnd.login_action_type == 1) || App.MainWnd.login_action_type == 0)
            {
                btn_StartGame.IsEnabled = false;
                App.MainWnd.runGame();
                btn_StartGame.IsEnabled = true;
            }
            else
                btnGetOtp_Click(null, null);
        }

        private void autoPaste_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (bool.Parse(ConfigAppSettings.GetValue("autoPaste", "false")) == autoPaste.IsChecked)
                return;
            if (ConfigAppSettings.GetValue("autoPaste", "") == "")
                MessageBox.Show("自動輸入需要滿足以下條件才能正常使用:\r\n1.遊戲需要在輸入帳密界面\r\n2.遊戲沒有選中記住帳號\r\n3.遊戲帳號密碼輸入欄為空\r\n4.輸入欄激活狀態為帳號欄位\r\n\r\n※ 自動輸入功能可能會由於遊戲限制出現偶爾無法正常進行的問題, 請斟酌使用");
            ConfigAppSettings.SetValue("autoPaste", Convert.ToString(autoPaste.IsChecked));
        }

        public void btnGetOtp_Click(object sender, RoutedEventArgs e)
        {
            if (list_Account.SelectedIndex < 0 || App.MainWnd.loginWorker.IsBusy)
            {
                MessageBox.Show("您還未選擇需要啟動遊戲的帳號。");
                return;
            }

            this.btnGetOtp.Content = "正在獲取";
            this.t_Password.Text = "";
            this.list_Account.IsEnabled = false;
            this.btnGetOtp.IsEnabled = false;
            this.btn_Logout.IsEnabled = false;
            this.btn_ChangeGame.IsEnabled = false;
            this.gameName.IsEnabled = false;
            this.btn_StartGame.IsEnabled = false;
            this.btnAddServiceAccount.IsEnabled = false;
            this.m_MenuList.IsEnabled = false;
            App.MainWnd.getOtpWorker.RunWorkerAsync(list_Account.SelectedIndex);
        }

        private void t_Password_PreviewMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (t_Password.Text == "" || (string)btnGetOtp.Content == "正在獲取") return;
            try
            {
                Clipboard.SetText(t_Password.Text);
            }
            catch { }
        }

        private void btnAddServiceAccount_Click(object sender, RoutedEventArgs e)
        {
            if (!btnAddServiceAccount.IsEnabled)
                return;
            if ((string) btnAddServiceAccount.Content == "前往認證")
            {
                new WebBrowser("https://tw.beanfun.com/TW/member/verify_index.aspx").Show();
            }
            else
            {
                if (App.MainWnd.service_code == "610153" && App.MainWnd.service_region == "TN" || App.MainWnd.service_code == "610085" && App.MainWnd.service_region == "TC") new UnconnectedGame_AddAccount().ShowDialog();
                else new AddServiceAccount().ShowDialog();
            }
        }

        private void m_UpdatePoint_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.updateRemainPoint(App.MainWnd.bfClient.getRemainPoint());
        }

        private void m_ChangeAccName_Click(object sender, RoutedEventArgs e)
        {
            BeanfunClient.ServiceAccount account = (BeanfunClient.ServiceAccount)list_Account.SelectedItem;
            if (account == null)
                return;
            new ChangeServiceAccountDisplayName(account.sname).ShowDialog();
        }

        private void bfb_Gash_Click(object sender, RoutedEventArgs e)
        {
            string url;
            if (App.LoginRegion == "TW")
            {
                url = $"https://tw.beanfun.com/TW/auth.aspx?channel=gash&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token={ App.MainWnd.bfClient.WebToken }";
            }
            else
            {
                url = $"https://hk.beanfun.com/beanfun_web_ap/auth.aspx?channel=gash&page_and_query=default.aspx%3fservice_code%3d999999%26service_region%3dT0&token={ App.MainWnd.bfClient.BFServ.Token }";
            }
            new WebBrowser(url).Show();
        }

        private void BF_btnMember_Click(object sender, RoutedEventArgs e)
        {
            string url;
            if (App.LoginRegion == "TW")
            {
                url = $"https://tw.beanfun.com/TW/auth.aspx?channel=member&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token={ App.MainWnd.bfClient.WebToken }";
            }
            else
            {
                url = $"https://hk.beanfun.com/beanfun_web_ap/auth.aspx?channel=member&page_and_query=default.aspx%3fservice_code%3d999999%26service_region%3dT0&token={ App.MainWnd.bfClient.BFServ.Token }";
            }
            new WebBrowser(url).Show();
        }

        private void btn_Customerservice_Click(object sender, RoutedEventArgs e)
        {
            string url;
            if (App.LoginRegion == "TW")
            {
                url = "https://tw.beanfun.com/customerservice/www/main.aspx";
            }
            else
            {
                url = "http://hk.games.beanfun.com/faq/service.asp";
            }
            new WebBrowser(url).Show();
        }

        private void m_GetEmail_Click(object sender, RoutedEventArgs e)
        {
            new CopyBox("認證信箱", App.MainWnd.bfClient.getEmail()).ShowDialog();
        }

        private void m_AccInfo_Click(object sender, RoutedEventArgs e)
        {
            BeanfunClient.ServiceAccount account = (BeanfunClient.ServiceAccount)list_Account.SelectedItem;
            if (account == null)
                return;
            new ServiceAccountInfo(account).ShowDialog();
        }

        private void btn_HomePage_Click(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd != null && App.MainWnd.SelectedGame != null)
                new WebBrowser(App.MainWnd.SelectedGame.website_url).Show();
        }

        private void gameName_Click(object sender, RoutedEventArgs e)
        {
            new GameList().ShowDialog();
        }

        private void m_ChangePassword_Click(object sender, RoutedEventArgs e)
        {
            new UnconnectedGame_ChangePassword().ShowDialog();
        }

        private void btn_Tools_Click(object sender, RoutedEventArgs e)
        {
            string gameCode = App.MainWnd.service_code + "_" + App.MainWnd.service_region;
            switch (gameCode)
            {
                case "610074_T9":
                    new MapleTools().Show();
                    break;
                case "610096_TE":
                    new KartTools().Show();
                    break;
            }
        }

        private void btn_Deposite_Click(object sender, RoutedEventArgs e)
        {
            new WebBrowser("https://m.beanfun.com/Deposite").Show();
        }
    }
}
