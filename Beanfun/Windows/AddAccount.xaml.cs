using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// AddAccount.xaml 的交互逻辑
    /// </summary>
    public partial class AddAccount : Window
    {
        public AddAccount()
        {
            InitializeComponent();
            if (!App.IsWin10) SourceChord.FluentWPF.AcrylicWindow.SetTintOpacity(this, 1.0);
            initPage();
        }

        private void initPage()
        {
            string s_Region = region.SelectedIndex == 0 ? "TW" : "HK";
            b_Method.Items.Clear();
            if (s_Region == "TW")
            {
                foreach (string type in App.MainWnd.loginPage.item_TW)
                {
                    if (type == "QR Code便利登") continue;
                    b_Method.Items.Add(type);
                }
                t_Verify.Visibility = Visibility.Visible;
            }
            else
            {
                foreach (string type in App.MainWnd.loginPage.item_HK)
                {
                    b_Method.Items.Add(type);
                }
                t_Verify.Visibility = Visibility.Collapsed;
                t_Verify.Text = "";
            }
            b_Method.SelectedIndex = 0;
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void region_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if (b_Method != null)
                initPage();
        }

        private void method_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            string s_Region = region.SelectedIndex == 0 ? "TW" : "HK";
            if (s_Region == "HK" && b_Method.SelectedIndex > 0)
            {
                b_Method.SelectedIndex = 0;
            }
            t_Password.Text = "";
            autoLogin.IsChecked = false;
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (t_AccountID.Text == null || t_AccountID.Text == "")
            {
                MessageBox.Show("請輸入帳號");
                return;
            }
            App.MainWnd.accountManager.addAccount(region.SelectedIndex == 0 ? "TW" : "HK", t_AccountID.Text, t_AccountName.Text, t_Password.Text, t_Verify.Text, b_Method.SelectedIndex, t_Password.Text == "" ? false : (bool)autoLogin.IsChecked);
            App.MainWnd.ddlAuthTypeItemsInit();
            this.Close();
        }
    }
}
