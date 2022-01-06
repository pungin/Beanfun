using System.Windows;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// UnconnectedGame_AddAccount.xaml 的交互逻辑
    /// </summary>
    public partial class UnconnectedGame_AddAccount : Window
    {
        private System.Collections.Specialized.NameValueCollection payload = null;

        public UnconnectedGame_AddAccount()
        {
            payload = App.MainWnd.UnconnectedGame_AddAccountInit();
            if (payload == null)
            {
                MessageBox.Show(TryFindResource("UnknownError") as string, TryFindResource("SystemInfo") as string);
                this.Close();
                return;
            }

            InitializeComponent();

            string gameName = payload.Get("GameName");
            string accountLen = payload.Get("AccountLen");
            payload.Remove("GameName");
            payload.Remove("AccountLen");
            if (payload.Get("CheckNickName") == "")
            {
                DNtr.Visibility = Visibility.Collapsed;
                lbtnCheckNickName.Visibility = Visibility.Collapsed;
            }
            payload.Remove("CheckNickName");

            lblAccountLen.Text = accountLen;
            lblGameName.Text = gameName;
            lblGameName1.Text = gameName;
            lblGameName2.Text = gameName;
            lblGameName3.Text = gameName;
            lblGameName4.Text = gameName;
            lblGameName5.Text = gameName;
            lbtnGameName.Text = gameName;
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void Hyperlink_Click(object sender, RoutedEventArgs e)
        {
            payload = App.MainWnd.UnconnectedGame_AddUnconnectedCheck(txtServiceAccountID.Text, DNtr.Visibility == Visibility.Visible ? "" : null, payload);
            if (payload == null || payload.Get("lblErrorMessage") == "")
            {
                payload = null;
                MessageBox.Show(TryFindResource("UnknownError") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            lblErrorMessage.Visibility = Visibility.Visible;
            lblErrorMessage.Content = payload.Get("lblErrorMessage");
            payload.Remove("lblErrorMessage");
        }

        private void Hyperlink_Click_1(object sender, RoutedEventArgs e)
        {
            if (lbtnCheckNickName.Visibility != Visibility.Visible) return;
            payload = App.MainWnd.UnconnectedGame_AddAccountCheckNickName(txtServiceAccountDN.Text, payload);
            if (payload == null || payload.Get("lblErrorMessage") == "")
            {
                payload = null;
                MessageBox.Show(TryFindResource("UnknownError") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            lblErrorMessage.Visibility = Visibility.Visible;
            lblErrorMessage.Content = payload.Get("lblErrorMessage");
            payload.Remove("lblErrorMessage");
        }

        private void Hyperlink_Click_2(object sender, RoutedEventArgs e)
        {
            string contract = App.MainWnd.GetServiceContract();
            if (contract == "")
            {
                MessageBox.Show(TryFindResource("UnknownError") as string, TryFindResource("SystemInfo") as string);
            }
            else
            {
                new Contract(contract).ShowDialog();
            }
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            string sAccountLen = lblAccountLen.Text;
            if (sAccountLen == null || sAccountLen == "" || !sAccountLen.Contains(" - "))
            {
                MessageBox.Show(TryFindResource("UnknownError") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            string[] aAccountLen = sAccountLen.Split(new string[] { " - " }, System.StringSplitOptions.None);
            byte accountLenMin = byte.Parse(aAccountLen[0]);
            byte accountLenMax = byte.Parse(aAccountLen[1]);
            if (txtServiceAccountID.Text == null || txtServiceAccountID.Text == "")
            {
                MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_18") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            if (txtServiceAccountID.Text.Length < accountLenMin || txtServiceAccountID.Text.Length > accountLenMax)
            {
                MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_19") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            if (txtNewPwd.Password == null || txtNewPwd.Password == "")
            {
                MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_20") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            if (txtNewPwd.Password.Length < accountLenMin || txtNewPwd.Password.Length > accountLenMax)
            {
                MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_21") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            if (txtNewPwd2.Password == null || txtNewPwd2.Password == "")
            {
                MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_22") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            if (txtNewPwd2.Password.Length < accountLenMin || txtNewPwd2.Password.Length > accountLenMax)
            {
                MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_23") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            if (DNtr.Visibility == Visibility.Visible)
            {
                if (txtServiceAccountDN.Text == null || txtServiceAccountDN.Text == "")
                {
                    MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_24") as string, TryFindResource("SystemInfo") as string);
                    return;
                }
                if (txtServiceAccountDN.Text.Length < 2 || txtServiceAccountDN.Text.Length > 6)
                {
                    MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_25") as string, TryFindResource("SystemInfo") as string);
                    return;
                }
            }
            if (!(bool)chkBox1.IsChecked)
            {
                MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_26") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            string result = App.MainWnd.UnconnectedGame_AddAccount(txtServiceAccountID.Text, txtNewPwd.Password, txtNewPwd2.Password, DNtr.Visibility == Visibility.Visible ? txtServiceAccountDN.Text : null, payload);
            if (result == "") this.Close();
            else if (result == null) MessageBox.Show(TryFindResource("UnconnectedGame_AddAccount_27") as string, TryFindResource("SystemInfo") as string);
            else
            {
                lblErrorMessage.Visibility = Visibility.Visible;
                lblErrorMessage.Content = result;
            }
        }
    }
}
