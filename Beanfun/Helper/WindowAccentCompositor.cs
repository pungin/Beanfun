using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;

namespace Beanfun
{
    /// <summary>
    /// 为窗口提供模糊特效。
    /// </summary>
    public class WindowAccentCompositor
    {
        private readonly Window _window;
        private bool _isEnabled;
        private int _blurColor;

        /// <summary>
        /// 创建 <see cref="WindowAccentCompositor"/> 的一个新实例。
        /// </summary>
        /// <param name="window">要创建模糊特效的窗口实例。</param>
        public WindowAccentCompositor(Window window) => _window = window ?? throw new ArgumentNullException(nameof(window));

        /// <summary>
        /// 获取或设置此窗口模糊特效是否生效的一个状态。
        /// 默认为 false，即不生效。
        /// </summary>
        [DefaultValue(false)]
        public bool IsEnabled
        {
            get => _isEnabled;
            set
            {
                _isEnabled = value;
                OnIsEnabledChanged(value);
            }
        }

        /// <summary>
        /// 获取或设置此窗口模糊特效叠加的颜色。
        /// </summary>
        public Color Color
        {
            get => Color.FromArgb(
                // 取出红色分量。
                (byte)((_blurColor & 0x000000ff) >> 0),
                // 取出绿色分量。
                (byte)((_blurColor & 0x0000ff00) >> 8),
                // 取出蓝色分量。
                (byte)((_blurColor & 0x00ff0000) >> 16),
                // 取出透明分量。
                (byte)((_blurColor & 0xff000000) >> 24));
            set => _blurColor =
                // 组装红色分量。
                value.R << 0 |
                // 组装绿色分量。
                value.G << 8 |
                // 组装蓝色分量。
                value.B << 16 |
                // 组装透明分量。
                value.A << 24;
        }

        private void OnIsEnabledChanged(bool isEnabled)
        {
            Window window = _window;
            var handle = new WindowInteropHelper(window).EnsureHandle();
            Composite(handle, isEnabled);
        }

        private void Composite(IntPtr handle, bool isEnabled)
        {
            // 创建 AccentPolicy 对象。
            var accent = new WindowsAPI.AccentPolicy();

            // 设置特效。
            if (!isEnabled)
            {
                accent.AccentState = WindowsAPI.AccentState.ACCENT_DISABLED;
            }
            else if (App.OSVersion >= App.Win11)
            {
                // 如果系统在 Windows 11 以上，则启用亚克力效果，并组合已设置的叠加颜色和透明度。
                // ※從 Windows 10 (1809) 開始已經支持亞克力效果但會造成窗口移動時卡頓問題
                //  请参见《在 WPF 程序中应用 Windows 10 真•亚克力效果》
                //  https://blog.walterlv.com/post/using-acrylic-in-wpf-application.html
                accent.AccentState = WindowsAPI.AccentState.ACCENT_ENABLE_ACRYLICBLURBEHIND;
                accent.GradientColor = _blurColor;
            }
            else if (App.OSVersion >= App.Win10)
            {
                // 如果系统在 Windows 10 以上，则启用 Windows 10 早期的模糊特效。
                // ※Windows 11 上使用模糊特效會造成窗口移動時卡頓問題
                //  请参见《在 Windows 10 上为 WPF 窗口添加模糊特效》
                //  https://blog.walterlv.com/post/win10/2017/10/02/wpf-transparent-blur-in-windows-10.html
                accent.AccentState = WindowsAPI.AccentState.ACCENT_ENABLE_BLURBEHIND;
            }
            else
            {
                // 暂时不处理其他操作系统：
                //  - Windows 8/8.1 不支持任何模糊特效
                //  - Windows Vista/7 支持 Aero 毛玻璃效果
                return;
            }

            // 将托管结构转换为非托管对象。
            var accentPolicySize = Marshal.SizeOf(accent);
            var accentPtr = Marshal.AllocHGlobal(accentPolicySize);
            Marshal.StructureToPtr(accent, accentPtr, false);

            // 设置窗口组合特性。
            try
            {
                // 设置模糊特效。
                var data = new WindowsAPI.WindowCompositionAttributeData
                {
                    Attribute = WindowsAPI.WindowCompositionAttribute.WCA_ACCENT_POLICY,
                    SizeOfData = accentPolicySize,
                    Data = accentPtr,
                };
                WindowsAPI.SetWindowCompositionAttribute(handle, ref data);
            }
            finally
            {
                // 释放非托管对象。
                Marshal.FreeHGlobal(accentPtr);
            }
        }
    }
}
