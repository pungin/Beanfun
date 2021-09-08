using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
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
                MessageBox.Show("未知錯誤, 更變遊戲帳號名失敗。", "系統訊息");
            }
        }

        private void ButtonCancel_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }
    }
}
