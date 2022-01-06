using System.Windows;
using System.Windows.Input;

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
                MessageBox.Show(TryFindResource("MsgDisplayNameNeed") as string, TryFindResource("SystemInfo") as string);
                return;
            } else if (!(bool)cbContract.IsChecked)
            {
                MessageBox.Show(TryFindResource("MsgTermsOfServiceNeed") as string, TryFindResource("SystemInfo") as string);
                return;
            }
            this.Close();
            if (!App.MainWnd.AddServiceAccount(txtNewServiceAccountDisplayName.Text))
            {
                MessageBox.Show(TryFindResource("MsgCreateServiceAccountFailed") as string, TryFindResource("SystemInfo") as string);
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
                MessageBox.Show(TryFindResource("UnknownError") as string, TryFindResource("SystemInfo") as string);
            }
            else
            {
                new Contract(contract).ShowDialog();
            }
        }
    }
}
