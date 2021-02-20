using System;
using System.Runtime.InteropServices;
using System.Text;

namespace Beanfun
{
    static class WindowsAPI
    {
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out int lpdwProcessId);

        [DllImport("user32.dll")]
        public static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern byte MapVirtualKey(byte wCode, int wMap);

        public static void PostString(IntPtr hwnd, string input)
        {
            const int WM_CHAR = 0x102;
            byte[] chars = ASCIIEncoding.ASCII.GetBytes(input);
            foreach (byte ch in chars)
            {
                PostMessage(hwnd, WM_CHAR, ch, 0);
            }
        }

        public static void PostKey(IntPtr hWnd, uint wMsg, byte wParam)
        {
            PostMessage(hWnd, wMsg, wParam, MapVirtualKey(wParam, 0) << 16 + 1);
        }

        [DllImport("user32.dll")]
        public static extern int PostMessage(IntPtr hWnd, uint wMsg, int wParam, int lParam);

        [DllImport("user32.dll")]
        public static extern bool ClientToScreen(IntPtr hWnd, ref System.Drawing.Point lpPoint);

        [DllImport("user32.dll")]
        public static extern bool GetCursorPos(ref System.Drawing.Point lpPoint);

        [DllImport("user32.dll")]
        public static extern int SetCursorPos(int x, int y);

        [DllImport("kernel32.dll", CharSet = CharSet.Auto)]
        private static extern Int32 GetSystemDefaultLocaleName([Out] StringBuilder lpLocaleName, Int32 cchLocaleName);
        public const Int32 LOCALE_NAME_MAX_LENGTH = 85;
        public static String GetSystemDefaultLocaleName()
        {
            StringBuilder lpLocaleName = new StringBuilder(LOCALE_NAME_MAX_LENGTH);
            if (GetSystemDefaultLocaleName(lpLocaleName, LOCALE_NAME_MAX_LENGTH) > 0)
            {
                return lpLocaleName.ToString();
            }

            return null;
        }

        [DllImport("kernel32.dll")]
        public static extern IntPtr GetCurrentProcess();

        [DllImport("kernel32.dll", CharSet = CharSet.Auto)]
        public static extern IntPtr GetModuleHandle(string moduleName);

        [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
        public static extern IntPtr GetProcAddress(IntPtr hModule, [MarshalAs(UnmanagedType.LPStr)] string procName);

        [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool IsWow64Process(IntPtr hProcess, out bool wow64Process);

        [DllImport("Kernel32.dll")]
        public static extern bool AttachConsole(int processId);
    }
}
