using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;

namespace Beanfun
{
    /// <summary>
    /// ChangeAccount.xaml 的互動邏輯
    /// </summary>
    public partial class ChangeAccount : Window
    {
        string region, account;
        int changedIndex;

        public ChangeAccount(int changedIndex, string region, string account)
        {
            this.region = region;
            this.account = account;
            this.changedIndex = changedIndex;
            InitializeComponent();
            t_AccountID.Text = account;
            t_AccountName.Text = App.MainWnd.accountManager.getNameByAccount(region, account);
            autoLogin.IsChecked = App.MainWnd.accountManager.getAutoLoginByAccount(region, account);
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (t_AccountID.Text == null || t_AccountID.Text == "")
            {
                MessageBox.Show("請輸入帳號");
                return;
            }
            string pwd = App.MainWnd.accountManager.getPasswordByAccount(region, account);
            string verify = App.MainWnd.accountManager.getVerifyByAccount(region, account);
            int method = App.MainWnd.accountManager.getMethodByAccount(region, account);
            App.MainWnd.accountManager.removeAccount(region, account);
            App.MainWnd.accountManager.addAccount(changedIndex, region, t_AccountID.Text, t_AccountName.Text, pwd, verify, method, (bool)autoLogin.IsChecked);
            App.MainWnd.ddlAuthTypeItemsInit();
            this.Close();
        }
    }
}
