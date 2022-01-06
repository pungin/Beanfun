using System;
using System.Security.Cryptography;
using System.Text;
using System.Windows;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// AccRecovery.xaml 的交互逻辑
    /// </summary>
    public partial class AccRecovery : Window
    {
        private AccountManager accMan;
        public AccRecovery(AccountManager a)
        {
            InitializeComponent();

            accMan = a;
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void Export_Button_Click(object sender, RoutedEventArgs e)
        {
            string plaintext = accMan.exportRecord();
            byte[] plain_bytes = Encoding.UTF8.GetBytes(plaintext);

            var md5 = new MD5CryptoServiceProvider();
            byte[] key = md5.ComputeHash(Encoding.UTF8.GetBytes(t_Password.Text));
            RijndaelManaged provider_AES = new RijndaelManaged();
            ICryptoTransform encrypt_AES = provider_AES.CreateEncryptor(key, md5.ComputeHash(Encoding.UTF8.GetBytes("pungin")));

            byte[] output = encrypt_AES.TransformFinalBlock(plain_bytes, 0, plain_bytes.Length);
            t_Data.Text = Convert.ToBase64String(output);

            MessageBox.Show(TryFindResource("ExportDone") as string);
        }

        private void Recovery_Button_Click(object sender, RoutedEventArgs e)
        {
            byte[] byte_pwd = Encoding.UTF8.GetBytes(t_Password.Text);
            MD5CryptoServiceProvider provider_MD5 = new MD5CryptoServiceProvider();
            byte[] byte_pwdMD5 = provider_MD5.ComputeHash(byte_pwd);

            RijndaelManaged provider_AES = new RijndaelManaged();
            ICryptoTransform decrypt_AES = provider_AES.CreateDecryptor(byte_pwdMD5, provider_MD5.ComputeHash(Encoding.UTF8.GetBytes("pungin")));

            byte[] input = Convert.FromBase64String(t_Data.Text);
            try
            {
                byte[] byte_secretContent = decrypt_AES.TransformFinalBlock(input, 0, input.Length);
                string plaintext = Encoding.UTF8.GetString(byte_secretContent);
                if (false == accMan.importRecord(plaintext))
                {
                    MessageBox.Show(TryFindResource("RecoveryFailed") as string);
                }
                else
                {
                    MessageBox.Show(TryFindResource("RecoverySuccess") as string);
                    App.MainWnd.loginMethodInit();
                }
            }
            catch
            {
                MessageBox.Show(TryFindResource("MsgDecryptFailed") as string);
                return;
            }
        }
    }
}
