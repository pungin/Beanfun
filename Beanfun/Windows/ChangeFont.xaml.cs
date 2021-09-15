using Microsoft.Win32;
using System;
using System.IO;
using System.ServiceProcess;
using System.Windows;
using System.Windows.Input;
using Utility.ModifyRegistry;

namespace Beanfun
{
    /// <summary>
    /// ChangeFont.xaml 的交互逻辑
    /// </summary>
    public partial class ChangeFont : Window
    {
        public ChangeFont()
        {
            InitializeComponent();
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void Run_MouseEnter(object sender, MouseEventArgs e)
        {
            Tips.Visibility = Visibility.Visible;
        }

        private void Run_MouseLeave(object sender, MouseEventArgs e)
        {
            Tips.Visibility = Visibility.Collapsed;
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            ModifyRegistry myRegistry = new ModifyRegistry();
            myRegistry.BaseRegistryKey = Registry.LocalMachine;
            myRegistry.SubKey = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
            myRegistry.Write("SimSun & NSimSun (TrueType)", "mingliu.ttc");

            myRegistry.SubKey = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\FontSubstitutes";
            myRegistry.Write("Simsun", "MingLiU");
            myRegistry.Write("NSimsun", "PMingLiU");

            MessageBox.Show("變更完成，請登出Windows帳戶後重新登入。");
        }

        private void Button_Click_1(object sender, RoutedEventArgs e)
        {
            try
            {
                ServiceController serviceController = new ServiceController("Windows Font Cache Service");
                if (serviceController.CanStop)
                {
                    serviceController.Stop();
                    serviceController.WaitForStatus(ServiceControllerStatus.Stopped, new TimeSpan(5000));
                    Directory.Delete("C:\\Windows\\ServiceProfiles\\LocalService\\AppData\\Local\\FontCache", true);
                }
            } catch {
                MessageBox.Show("失敗，請稍後再試");
            }
        }

        private void Button_Click_2(object sender, RoutedEventArgs e)
        {
            ModifyRegistry myRegistry = new ModifyRegistry();
            myRegistry.BaseRegistryKey = Registry.LocalMachine;
            myRegistry.SubKey = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
            myRegistry.Write("SimSun & NSimSun (TrueType)", "simsun.ttc");

            myRegistry.SubKey = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\FontSubstitutes";
            myRegistry.DeleteKey("Simsun");
            myRegistry.DeleteKey("NSimsun");

            MessageBox.Show("變更完成，請登出Windows帳戶後重新登入。");
        }
    }
}
