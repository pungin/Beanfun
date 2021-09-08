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
            if (result == null) MessageBox.Show("未知錯誤。", "系統訊息");
            else if (result.StartsWith("verify_code"))
            {
                MessageBox.Show("請至您已認證的e - mail信箱中收取密碼設定信喲！\r\n確認碼: " + result.Replace("verify_code", "") + "\r\n為保障安全!請您在收到信後，\r\n點選連結前先確認信中的確認碼是否相同正確喔！", "資料已寄出！");
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
