using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// LoginTotp.xaml 的交互逻辑
    /// </summary>
    public partial class LoginTotp : Page
    {
        private TextBox[] _otpBoxes;
        private bool _isPasting;

        public LoginTotp()
        {
            InitializeComponent();
            _otpBoxes = new[] { otp1, otp2, otp3, otp4, otp5, otp6 };
        }

        private void btn_login_Click(object sender, RoutedEventArgs e)
        {
            btn_login.IsEnabled = false;
            btn_cancel.IsEnabled = false;
            App.MainWnd.do_Totp();
        }

        private void btn_back_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.frame.Content = App.MainWnd.loginPage;
        }

        private void otp_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            var box = sender as TextBox;
            int index = System.Array.IndexOf(_otpBoxes, box);

            if (e.Key == Key.Back && box.Text.Length == 0 && index > 0)
            {
                _otpBoxes[index - 1].Text = "";
                _otpBoxes[index - 1].Focus();
                e.Handled = true;
                return;
            }

            if (
                e.Key == Key.V
                && (Keyboard.Modifiers & ModifierKeys.Control) == ModifierKeys.Control
            )
            {
                e.Handled = true;
                HandlePaste();
            }
        }

        private void HandlePaste()
        {
            string text = "";
            try
            {
                text = Clipboard.GetText();
            }
            catch
            {
                return;
            }
            string digits = new string(text.Where(char.IsDigit).ToArray());
            if (digits.Length == 0)
                return;

            _isPasting = true;
            for (int i = 0; i < _otpBoxes.Length && i < digits.Length; i++)
            {
                _otpBoxes[i].Text = digits[i].ToString();
            }
            _isPasting = false;

            int focusIndex = System.Math.Min(digits.Length, _otpBoxes.Length - 1);
            _otpBoxes[focusIndex].Focus();

            TryAutoSubmit();
        }

        private void otp_TextChanged(object sender, TextChangedEventArgs e)
        {
            if (_isPasting)
                return;

            var box = sender as TextBox;
            int index = System.Array.IndexOf(_otpBoxes, box);

            // Filter non-digit
            string digits = new string(box.Text.Where(char.IsDigit).ToArray());
            if (digits != box.Text)
            {
                box.Text = digits;
                box.CaretIndex = digits.Length;
                return;
            }

            if (box.Text.Length == 1 && index < _otpBoxes.Length - 1)
            {
                _otpBoxes[index + 1].Focus();
            }

            TryAutoSubmit();
        }

        private void otp_GotFocus(object sender, RoutedEventArgs e)
        {
            TextBox box = sender as TextBox;
            box.SelectAll();
        }

        private void TryAutoSubmit()
        {
            if (btn_login.IsEnabled && _otpBoxes.All(b => b.Text.Length == 1))
            {
                btn_login.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            }
        }
    }
}
