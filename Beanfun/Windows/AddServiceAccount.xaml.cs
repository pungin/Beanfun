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
    /// AddServiceAccount.xaml 的交互逻辑
    /// </summary>
    public partial class AddServiceAccount : Window
    {
        public AddServiceAccount()
        {
            InitializeComponent();
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void ButtonOk_Click(object sender, RoutedEventArgs e)
        {
            if (txtNewServiceAccountDisplayName.Text == null || txtNewServiceAccountDisplayName.Text == "")
            {
                MessageBox.Show("請輸入使用者名稱！", "系統訊息");
                return;
            } else if (!(bool)cbContract.IsChecked)
            {
                MessageBox.Show("您必須先同意服務條款才可新增帳號！", "系統訊息");
                return;
            }
            this.Close();
            if (!App.MainWnd.AddServiceAccount(txtNewServiceAccountDisplayName.Text))
            {
                MessageBox.Show("新增遊戲帳號失敗, 可能這個遊戲無法創建帳號。", "系統訊息");
            }
        }

        private void ButtonCancel_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }

        private void aContract_Click(object sender, RoutedEventArgs e)
        {
            string contract = App.MainWnd.GetServiceContract();
            if (contract == "")
            {
                MessageBox.Show("發生未知錯誤", "系統訊息");
            }
            else
            {
                new Contract(contract).ShowDialog();
            }
        }
    }
}
