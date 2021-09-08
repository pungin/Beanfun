using System.Windows;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// Donate.xaml 的交互逻辑
    /// </summary>
    public partial class Donate : Window
    {
        public Donate()
        {
            InitializeComponent();
        }

        private void AliPay_MouseEnter(object sender, MouseEventArgs e)
        {
            QRCode_AliPay.Visibility = Visibility.Visible;
        }

        private void AliPay_MouseLeave(object sender, MouseEventArgs e)
        {
            QRCode_AliPay.Visibility = Visibility.Collapsed;
        }

        private void QQ_MouseEnter(object sender, MouseEventArgs e)
        {
            QRCode_QQ.Visibility = Visibility.Visible;
        }

        private void QQ_MouseLeave(object sender, MouseEventArgs e)
        {
            QRCode_QQ.Visibility = Visibility.Collapsed;
        }

        private void WeChat_MouseEnter(object sender, MouseEventArgs e)
        {
            QRCode_WeChat.Visibility = Visibility.Visible;
        }

        private void WeChat_MouseLeave(object sender, MouseEventArgs e)
        {
            QRCode_WeChat.Visibility = Visibility.Collapsed;
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }
    }
}
