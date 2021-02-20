using System;
using System.Text;
using System.Collections.Specialized;
using System.Drawing;
using System.IO;
using System.Net;
using System.Text.RegularExpressions;
using System.Windows.Media.Imaging;

namespace Beanfun
{
    public partial class BeanfunClient : WebClient
    {
        public string getVerifyPageInfo()
        {
            try
            {
                return this.DownloadString("https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx");
            }
            catch (Exception e)
            {
                this.errmsg = "VerifyUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }

        public BitmapImage getVerifyCaptcha(string samplecaptcha)
        {
            BitmapImage result;
            try
            {
                byte[] buffer = this.DownloadData("https://tw.newlogin.beanfun.com/LoginCheck/BotDetectCaptcha.ashx?get=image&c=c_logincheck_advancecheck_samplecaptcha&t=" + samplecaptcha);
                result = new BitmapImage();
                result.BeginInit();  
                result.StreamSource = new MemoryStream(buffer);
                result.EndInit();  
            }
            catch (Exception)
            {
                result = null;
            }
            return result;
        }

        public string verify(string viewstate, string eventvalidation, string samplecaptcha, string verifyCode, string captchaCode)
        {
            try
            {
                NameValueCollection payload = new NameValueCollection();
                payload.Add("__VIEWSTATE", viewstate);
                payload.Add("__EVENTVALIDATION", eventvalidation);
                payload.Add("txtVerify", verifyCode);
                payload.Add("CodeTextBox", captchaCode);
                payload.Add("imgbtnSubmit.x", "19");
                payload.Add("imgbtnSubmit.y", "23");
                payload.Add("LBD_VCID_c_logincheck_advancecheck_samplecaptcha", samplecaptcha);
                return this.UploadString("https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx", payload);
            }
            catch (Exception e)
            {
                this.errmsg = "VerifyUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }
    }
}
