using System.Windows;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// ChangeServiceAccountDisplayName.xaml 的交互逻辑
    /// </summary>
    public partial class ChangeServiceAccountDisplayName : Window
    {
        public ChangeServiceAccountDisplayName(string name)
        {
            InitializeComponent();
            txtNewServiceAccountDisplayName.Text = name;
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void ButtonOk_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
            if (!App.MainWnd.ChangeServiceAccountDisplayName(txtNewServiceAccountDisplayName.Text))
            {
                MessageBox.Show(TryFindResource("MsgChangeDisplayNameError") as string, TryFindResource("SystemInfo") as string);
            }
        }

        private void ButtonCancel_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }
    }
}
