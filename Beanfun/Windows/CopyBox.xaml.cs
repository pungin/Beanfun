using System.Windows;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// CopyBox.xaml 的交互逻辑
    /// </summary>
    public partial class CopyBox : Window
    {
        public CopyBox(string title, string value)
        {
            InitializeComponent();
            this.Title = title;
            t_Value.Text = value;
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                Clipboard.SetText(t_Value.Text);
                MessageBox.Show(TryFindResource("CopyFinished") as string);
            }
            catch
            {
                MessageBox.Show(TryFindResource("CopyFailed") as string);
            }
        }
    }
}
