using System;
using System.ComponentModel;
using System.Globalization;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Windows.Threading;

namespace Beanfun
{
    partial class WebBrowserHelper
    {
        private System.Windows.Controls.WebBrowser _webBrowser;
        private object _cookie;
        public event EventHandler<WebBrowserExtendedNavigatingEventArgs> BeforeNavigate;
        public event EventHandler<WebBrowserExtendedNavigatingEventArgs> BeforeNewWindow;

        public WebBrowserHelper(System.Windows.Controls.WebBrowser webBrowser)
        {
            if (webBrowser == null) throw new ArgumentNullException("webBrowser");
            _webBrowser = webBrowser;
            _webBrowser.Dispatcher.BeginInvoke(new Action(Attach), DispatcherPriority.Loaded);
            _webBrowser.Navigated += new System.Windows.Navigation.NavigatedEventHandler(_webBrowser_Navigated);
            if (NeedZoom()) _webBrowser.Navigated += new System.Windows.Navigation.NavigatedEventHandler(_webBrowser_SetDpi);
        }

        void _webBrowser_Navigated(object sender, System.Windows.Navigation.NavigationEventArgs e)
        {
            SetSilent(_webBrowser, true); // make it silent
        }

        public void Disconnect()
        {
            if (_cookie != null)
            {
                _cookie.ReflectInvokeMethod("Disconnect", new Type[] { }, null);
                _cookie = null;
            }
        }

        private void Attach()
        {
            var axIWebBrowser2 = _webBrowser.ReflectGetProperty("AxIWebBrowser2");
            var webBrowserEvent = new WebBrowserEvent(this);
            var cookieType = typeof(System.Windows.Controls.WebBrowser).Assembly.GetType("MS.Internal.Controls.ConnectionPointCookie");
            _cookie = Activator.CreateInstance(
                cookieType,
                ReflectionService.BindingFlags,
                null,
                new[] { axIWebBrowser2, webBrowserEvent, typeof(DWebBrowserEvents2) },
                CultureInfo.CurrentUICulture);
        }

        private void OnBeforeNavigate(string URL, string TargetFrameName, out bool Cancel)
        {
            var eventArgs = new WebBrowserExtendedNavigatingEventArgs(URL, TargetFrameName);
            if (null != BeforeNavigate)
            {
                BeforeNavigate(_webBrowser, eventArgs);
            }
            Cancel = eventArgs.Cancel;
        }

        private void OnBeforeNewWindow(string bstrUrl, out bool Cancel)
        {
            var eventArgs = new WebBrowserExtendedNavigatingEventArgs(bstrUrl, null);
            if (null != BeforeNewWindow)
            {
                BeforeNewWindow(_webBrowser, eventArgs);
            }
            Cancel = eventArgs.Cancel;
        }

        public static void SetSilent(System.Windows.Controls.WebBrowser browser, bool silent)
        {
            if (browser == null)
                throw new ArgumentNullException("browser");

            // get an IWebBrowser2 from the document
            IOleServiceProvider sp = browser.Document as IOleServiceProvider;
            if (sp != null)
            {
                Guid IID_IWebBrowserApp = new Guid("0002DF05-0000-0000-C000-000000000046");
                Guid IID_IWebBrowser2 = new Guid("D30C1661-CDAF-11d0-8A3E-00C04FC9E26E");

                object webBrowser;
                sp.QueryService(ref IID_IWebBrowserApp, ref IID_IWebBrowser2, out webBrowser);
                if (webBrowser != null)
                {
                    webBrowser.GetType().InvokeMember("Silent", BindingFlags.Instance | BindingFlags.Public | BindingFlags.PutDispProperty, null, webBrowser, new object[] { silent });
                }
            }
        }


        [ComImport, Guid("6D5140C1-7436-11CE-8034-00AA006009FA"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IOleServiceProvider
        {
            [PreserveSig]
            int QueryService([In] ref Guid guidService, [In] ref Guid riid, [MarshalAs(UnmanagedType.IDispatch)] out object ppvObject);
        }

        [DllImport("user32.dll")]
        static extern IntPtr GetDC(IntPtr ptr);

        [DllImport("user32.dll", EntryPoint = "ReleaseDC")]
        public static extern IntPtr ReleaseDC(IntPtr hWnd, IntPtr hDc);

        [DllImport("gdi32.dll")]
        public static extern IntPtr CreateDC(
            string lpszDriver,  // driver name
            string lpszDevice,  // device name
            string lpszOutput,  // not used; should be NULL
            Int64 lpInitData    // optional printer data
        );

        [DllImport("gdi32.dll")]
        public static extern int GetDeviceCaps(
            IntPtr hdc,         // handle to DC
            int nIndex          // index of capability
        );

        [DllImport("user32.dll")]
        public static extern bool SetProcessDPIAware();

        const int LOGPIXELSX = 88;
        const int LOGPIXELSY = 90;

        static System.Drawing.PointF GetCurrentDIPScale()
        {
            System.Drawing.PointF scaleUI = new System.Drawing.PointF(1.0f, 1.0f);
            try
            {
                SetProcessDPIAware();
                IntPtr screenDC = GetDC(IntPtr.Zero);
                int dpi_x = GetDeviceCaps(screenDC, LOGPIXELSX);
                int dpi_y = GetDeviceCaps(screenDC, LOGPIXELSY);

                scaleUI.X = (float)dpi_x / 96.0f;
                scaleUI.Y = (float)dpi_y / 96.0f;
                ReleaseDC(IntPtr.Zero, screenDC);
                return scaleUI;
            }
            catch {}

            return scaleUI;
        }

        /// <summary>
        /// The flags are used to zoom web browser's content.
        /// </summary>
        static readonly int OLECMDEXECOPT_DODEFAULT = 0;
        static readonly int OLECMDID_OPTICAL_ZOOM = 63;

        /// <summary>
        /// This function is used to zoom web browser's content.
        /// </summary>
        /// <param name="webbrowser">The instance of web browser.</param>
        /// <param name="zoom">The zoom scale. It should be 50~400</param>
        /// <remarks>This function must be invoked after the webbrowser has completely loaded the URI.</remarks>
        static void SetZoom(System.Windows.Controls.WebBrowser webbrowser, int zoom)
        {
            try
            {
                if (null == webbrowser)
                {
                    return;
                }

                FieldInfo fiComWebBrowser = webbrowser.GetType().GetField(
                    "_axIWebBrowser2", BindingFlags.Instance | BindingFlags.NonPublic);
                if (null != fiComWebBrowser)
                {
                    Object objComWebBrowser = fiComWebBrowser.GetValue(webbrowser);

                    if (null != objComWebBrowser)
                    {
                        object[] args = new object[]
                        {
                            OLECMDID_OPTICAL_ZOOM,
                            OLECMDEXECOPT_DODEFAULT,
                            zoom,
                            IntPtr.Zero
                        };
                        objComWebBrowser.GetType().InvokeMember(
                            "ExecWB",
                            BindingFlags.InvokeMethod,
                            null, objComWebBrowser,
                            args);
                    }
                }
            }
            catch {}
        }

        static void _webBrowser_SetDpi(object sender, System.Windows.Navigation.NavigationEventArgs e)
        {
            System.Windows.Controls.WebBrowser browser = sender as System.Windows.Controls.WebBrowser;
            if (null != browser)
            {
                browser.LoadCompleted -= new System.Windows.Navigation.LoadCompletedEventHandler(_webBrowser_SetDpi);
                System.Drawing.PointF scaleUI = GetCurrentDIPScale();
                if (100 != (int)(scaleUI.X * 100))
                {
                    SetZoom(browser, (int)(scaleUI.X * scaleUI.Y * 100));
                }
            }
        }

        static bool NeedZoom()
        {
            System.Drawing.PointF scaleUI = GetCurrentDIPScale();
            if (100 != (int)(scaleUI.X * 100))
            {
                return true;
            }

            return false;
        }
    }

    public static class ReflectionService
    {
        public readonly static BindingFlags BindingFlags =
            BindingFlags.Instance |
            BindingFlags.Public |
            BindingFlags.NonPublic |
            BindingFlags.FlattenHierarchy |
            BindingFlags.CreateInstance;

        public static object ReflectGetProperty(this object target, string propertyName)
        {
            if (target == null)
                throw new ArgumentNullException("target");
            if (string.IsNullOrWhiteSpace(propertyName))
                throw new ArgumentException("propertyName can not be null or whitespace", "propertyName");
            var propertyInfo = target.GetType().GetProperty(propertyName, BindingFlags);
            if (propertyInfo == null)
                throw new ArgumentException(string.Format("Can not find property '{0}' on '{1}'", propertyName, target.GetType()));
            return propertyInfo.GetValue(target, null);
        }

        public static object ReflectInvokeMethod(this object target, string methodName, Type[] argTypes, object[] parameters)
        {
            if (target == null)
                throw new ArgumentNullException("target");
            if (string.IsNullOrWhiteSpace(methodName))
                throw new ArgumentException("methodName can not be null or whitespace", "methodName");
            var methodInfo = target.GetType().GetMethod(methodName, BindingFlags, null, argTypes, null);
            if (methodInfo == null)
                throw new ArgumentException(string.Format("Can not find method '{0}' on '{1}'", methodName, target.GetType()));
            return methodInfo.Invoke(target, parameters);
        }
    }

    [ComImport, TypeLibType(TypeLibTypeFlags.FHidden), InterfaceType(ComInterfaceType.InterfaceIsIDispatch), Guid("34A715A0-6587-11D0-924A-0020AFC7AC4D")]
    public interface DWebBrowserEvents2
    {
        [DispId(0xFA)]
        void BeforeNavigate2([In, MarshalAs(UnmanagedType.IDispatch)] object pDisp, [In] ref object URL, [In] ref object Flags, [In] ref object TargetFrameName, [In] ref object PostData, [In] ref object Headers, [In, Out] ref bool Cancel);
        [DispId(0x111)]
        void NewWindow3([In, Out, MarshalAs(UnmanagedType.IDispatch)] ref  object ppDisp, [In, Out] ref bool Cancel, [In] ref object dwFlags, [In] ref object bstrUrlContext, [In] ref object bstrUrl);
    }

    public partial class WebBrowserHelper
    {
        private class WebBrowserEvent : StandardOleMarshalObject, DWebBrowserEvents2
        {
            private WebBrowserHelper _helperInstance = null;

            public WebBrowserEvent(WebBrowserHelper helperInstance)
            {
                _helperInstance = helperInstance;
            }

            #region DWebBrowserEvents2 Members

            public void BeforeNavigate2(object pDisp, ref object URL, ref object Flags, ref object TargetFrameName, ref object PostData, ref object Headers, ref bool Cancel)
            {
                _helperInstance.OnBeforeNavigate((string)URL, (string)TargetFrameName, out Cancel);
            }

            public void NewWindow3(ref object ppDisp, ref bool Cancel, ref object dwFlags, ref object bstrUrlContext, ref object bstrUrl)
            {
                _helperInstance.OnBeforeNewWindow((string)bstrUrl, out Cancel);
            }
            #endregion
        }
    }

    public class WebBrowserExtendedNavigatingEventArgs : CancelEventArgs
    {
        private string _Url;
        public string Url
        {
            get { return _Url; }
        }

        private string _Frame;
        public string Frame
        {
            get { return _Frame; }
        }

        public WebBrowserExtendedNavigatingEventArgs(string url, string frame) : base()
        {
            _Url = url;
            _Frame = frame;
        }
    }
}
