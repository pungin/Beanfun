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
            initPage();
        }

        private void initPage()
        {
            string s_Region = region.SelectedIndex == 0 ? "TW" : "HK";
            if (s_Region == "TW")
                t_Verify.Visibility = Visibility.Visible;
            else
            {
                t_Verify.Visibility = Visibility.Collapsed;
                t_Verify.Text = "";
            }
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void region_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if (t_Verify != null) initPage();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (t_AccountID.Text == null || t_AccountID.Text == "")
            {
                MessageBox.Show(TryFindResource("AccountNeed") as string);
                return;
            }
            App.MainWnd.accountManager.addAccount(region.SelectedIndex == 0 ? "TW" : "HK", t_AccountID.Text, t_AccountName.Text, t_Password.Text, t_Verify.Text, 0, t_Password.Text == "" ? false : (bool)autoLogin.IsChecked);
            App.MainWnd.loginMethodInit();
            this.Close();
        }
    }
}
