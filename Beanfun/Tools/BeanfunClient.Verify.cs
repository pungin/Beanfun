using System;
using System.Collections.Specialized;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Net;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows.Media.Imaging;

namespace Beanfun
{
    public partial class BeanfunClient : WebClient
    {
        // Stored from getVerifyPageInfo HTML parsing
        public string verifyFormAction;
        public string verifyViewStateGenerator;

        public string getVerifyPageInfo()
        {
            try
            {
                string url = !string.IsNullOrEmpty(this.advanceCheckUrl)
                    ? this.advanceCheckUrl
                    : "https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx";
                return this.DownloadString(url);
            }
            catch (Exception e)
            {
                this.errmsg = "VerifyUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }

        public BitmapImage getVerifyCaptcha(string samplecaptcha)
        {
            if (string.IsNullOrWhiteSpace(samplecaptcha))
                return null;

            BitmapImage result;
            try
            {
                string url =
                    "https://tw.newlogin.beanfun.com/LoginCheck/BotDetectCaptcha.ashx?get=image&c=c_logincheck_advancecheck_samplecaptcha&t="
                    + samplecaptcha;
                byte[] buffer = this.DownloadData(url);

                if (buffer == null || buffer.Length < 500)
                {
                    Debug.WriteLine("[Captcha] Downloaded data is too small to be an image.");
                    return null;
                }

                result = new BitmapImage();
                result.BeginInit();
                result.CacheOption = BitmapCacheOption.OnLoad;
                result.StreamSource = new MemoryStream(buffer);
                result.EndInit();
                result.Freeze();
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[Captcha] Error parsing image: {ex.Message}");
                result = null;
            }
            return result;
        }

        public string verify(
            string viewstate,
            string eventvalidation,
            string samplecaptcha,
            string verifyCode,
            string captchaCode
        )
        {
            try
            {
                NameValueCollection payload = new NameValueCollection();
                payload.Add("__VIEWSTATE", viewstate);
                if (!string.IsNullOrEmpty(this.verifyViewStateGenerator))
                    payload.Add("__VIEWSTATEGENERATOR", this.verifyViewStateGenerator);
                payload.Add("__EVENTVALIDATION", eventvalidation);
                payload.Add("txtVerify", verifyCode);
                payload.Add("CodeTextBox", captchaCode);
                payload.Add("imgbtnSubmit.x", "19");
                payload.Add("imgbtnSubmit.y", "23");
                payload.Add("LBD_VCID_c_logincheck_advancecheck_samplecaptcha", samplecaptcha);

                string submitUrl = !string.IsNullOrEmpty(this.verifyFormAction)
                    ? this.verifyFormAction
                    : "https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx";
                return this.UploadString(submitUrl, payload);
            }
            catch (Exception e)
            {
                this.errmsg = "VerifyUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }
    }
}
