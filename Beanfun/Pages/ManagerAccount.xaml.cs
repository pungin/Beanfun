using System.Collections.Generic;
using System.Windows;
using System.Windows.Controls;

namespace Beanfun
{
    /// <summary>
    /// ManagerAccount.xaml 的交互逻辑
    /// </summary>
    public partial class ManagerAccount : Page
    {
        public class BeanfunAccount
        {
            public string method { get; set; }
            public string account { get; set; }
            public string accountname { get; set; }
            public string isSavePwd { get; set; }
            public string isAutoLogin { get; set; }
            public string isSaveVerify { get; set; }

            public BeanfunAccount()
            { this.method = null; this.account = null; this.accountname = null; this.isSavePwd = null; this.isAutoLogin = null; this.isSaveVerify = null; }
            public BeanfunAccount(string method, string account, string accountname, string isSavePwd, string isAutoLogin, string isSaveVerify = null)
            { this.method = method; this.account = account; this.accountname = accountname; this.isSavePwd = isSavePwd; this.isAutoLogin = isAutoLogin; this.isSaveVerify = isSaveVerify; }
        }

        public ManagerAccount()
        {
            InitializeComponent();
        }

        public List<string> item_TW = new List<string>
        {
            "帳號密碼",
            "QR Code便利登"
        };
        public List<string> item_HK = new List<string>
        {
            "帳號密碼"
        };

        public void setupAccList(MainWindow MainWnd)
        {
            string region = !btn_TW.IsEnabled ? "TW" : "HK";
            List<string> methodItem = region == "TW" ? item_TW : item_HK;
            string[] accList = MainWnd.accountManager.getAccountList(region);
            List<BeanfunAccount> accountList = new List<BeanfunAccount>();
            foreach (string s in accList)
            {
                accountList.Add(new BeanfunAccount(
                    methodItem[MainWnd.accountManager.getMethodByAccount(region, s)],
                    s,
                    MainWnd.accountManager.getNameByAccount(region, s),
                    MainWnd.accountManager.getPasswordByAccount(region, s) != "" ? "是" : "否",
                    MainWnd.accountManager.getAutoLoginByAccount(region, s) ? "是" : "否",
                    MainWnd.accountManager.getVerifyByAccount(region, s) != "" ? "是" : "否"
                ));
            }
            list_Account.ItemsSource = null;
            list_Account.ItemsSource = accountList;
            if (accList.Length > 0)
                list_Account.SelectedIndex = 0;
            else
            {
                btn_Change.IsEnabled = false;
                btn_Delete.IsEnabled = false;
            }
        }

        private void TW_Button_Click(object sender, RoutedEventArgs e)
        {
            if (!btn_TW.IsEnabled)
                return;
            btn_TW.IsEnabled = false;
            btn_HK.IsEnabled = true;
            setupAccList(App.MainWnd);
        }

        private void HK_Button_Click(object sender, RoutedEventArgs e)
        {
            if (!btn_HK.IsEnabled)
                return;
            btn_TW.IsEnabled = true;
            btn_HK.IsEnabled = false;
            setupAccList(App.MainWnd);
        }

        private void Up_Button_Click(object sender, RoutedEventArgs e)
        {
            if (list_Account.SelectedIndex <= 0) return;
            changeAccountIndex(true);
        }

        private void Down_Button_Click(object sender, RoutedEventArgs e)
        {
            if (list_Account.SelectedIndex + 1 >= list_Account.Items.Count) return;
            changeAccountIndex(false);
        }

        private void changeAccountIndex(bool up)
        {
            string region = !btn_TW.IsEnabled ? "TW" : "HK";
            string account = ((BeanfunAccount)list_Account.SelectedItem).account;
            string name = App.MainWnd.accountManager.getNameByAccount(region, account);
            string password = App.MainWnd.accountManager.getPasswordByAccount(region, account);
            string verify = App.MainWnd.accountManager.getVerifyByAccount(region, account);
            int method = App.MainWnd.accountManager.getMethodByAccount(region, account);
            bool autoLogin = App.MainWnd.accountManager.getAutoLoginByAccount(region, account);
            int changedIndex = list_Account.SelectedIndex + (up ? -1 : 1);
            App.MainWnd.accountManager.addAccount(changedIndex, region, account, name, password, verify, method, autoLogin);
            setupAccList(App.MainWnd);
            App.MainWnd.loginMethodInit();
            list_Account.SelectedIndex = changedIndex;
        }

        private void Add_Button_Click(object sender, RoutedEventArgs e)
        {
            if (!btn_Add.IsEnabled)
                return;
            AddAccount wnd = new AddAccount();
            wnd.ShowDialog();
        }

        private void Delete_Button_Click(object sender, RoutedEventArgs e)
        {
            if (!btn_Delete.IsEnabled)
                return;

            string t_acc_del = "";
            if (list_Account.SelectedItems.Count < 1)
                return;
            else if(list_Account.SelectedItems.Count > 1)
                t_acc_del = $" {list_Account.SelectedItems.Count} 個帳號";
            else
                t_acc_del = $"帳號「{((BeanfunAccount)list_Account.SelectedItem).account}」";
            MessageBoxResult result = MessageBox.Show($"即將移除{t_acc_del}，此操作不可恢復，是否確認要移除？", "移除帳號", MessageBoxButton.YesNo);

            if (result == MessageBoxResult.Yes)
            {
                string region = !btn_TW.IsEnabled ? "TW" : "HK";
                foreach (BeanfunAccount acc in list_Account.SelectedItems)
                {
                    App.MainWnd.accountManager.removeAccount(region, acc.account);
                }
                setupAccList(App.MainWnd);
                App.MainWnd.loginMethodInit();
            }
        }

        private void Return_Button_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.frame.Content = App.MainWnd.settingPage;
        }

        private void list_Account_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            btn_Up.IsEnabled = false;
            btn_Down.IsEnabled = false;
            if (list_Account.Items.Count <= 0)
            {
                btn_Change.IsEnabled = false;
                btn_Delete.IsEnabled = false;
                return;
            }

            if (list_Account.SelectedIndex > 0)
                btn_Up.IsEnabled = true;

            if (list_Account.SelectedIndex + 1 < list_Account.Items.Count)
                btn_Down.IsEnabled = true;

            btn_Change.IsEnabled = true;
            btn_Delete.IsEnabled = true;
        }

        private void Change_Button_Click(object sender, RoutedEventArgs e)
        {
            if (!btn_Change.IsEnabled)
                return;
            ChangeAccount wnd = new ChangeAccount(list_Account.SelectedIndex, !btn_TW.IsEnabled ? "TW" : "HK", ((BeanfunAccount)list_Account.SelectedItem).account);
            wnd.ShowDialog();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            new AccRecovery(App.MainWnd.accountManager).ShowDialog();
        }
    }
}
