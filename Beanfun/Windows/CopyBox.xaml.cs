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
    /// CopyBox.xaml 的交互逻辑
    /// </summary>
    public partial class CopyBox : Window
    {
        public CopyBox(string title, string value)
        {
            InitializeComponent();
            if (!App.IsWin10) SourceChord.FluentWPF.AcrylicWindow.SetTintOpacity(this, 1.0);
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
                MessageBox.Show("複製完成");
            }
            catch
            {
                MessageBox.Show("複製失敗");
            }
        }
    }
}
