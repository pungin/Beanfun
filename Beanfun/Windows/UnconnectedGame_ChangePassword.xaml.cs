using System.Windows;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// UnconnectedGame_ChangePassword.xaml 的交互逻辑
    /// </summary>
    public partial class UnconnectedGame_ChangePassword : Window
    {
        public UnconnectedGame_ChangePassword()
        {
            InitializeComponent();
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            string result = App.MainWnd.UnconnectedGame_ChangePassword(txtEmail.Text);
            if (result == null) MessageBox.Show(TryFindResource("UnknownError") as string, TryFindResource("SystemInfo") as string);
            else if (result.StartsWith("verify_code"))
            {
                MessageBox.Show(string.Format((TryFindResource("MsgChangePassword") as string).Replace("\\r\\n", "\r\n"), result.Replace("verify_code", "")), TryFindResource("DataSended") as string);
                this.Close();
            }
            else
            {
                lblErrorMessage.Visibility = Visibility.Visible;
                lblErrorMessage.Content = result;
            }
        }
    }
}
