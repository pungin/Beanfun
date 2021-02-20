using Newtonsoft.Json;
using System;
using System.IO;
using System.Net;
using System.Net.NetworkInformation;
using System.Security.Cryptography;
using System.Text;
using System.Web;

namespace Beanfun
{
    public enum APIUrl
    {
        GetSk,
        GetRc,
        CreRl,
        CreLl
    }

    public class Response
    {
        public string Result
        { get; set; }

        public string ResultMessage
        { get; set; }

        public object ResultData
        { get; set; }

        public string ReturnValue
        { get; set; }

        public string ReturnMessage
        { get; set; }

        public object ReturnData
        { get; set; }
    }

    public class SKIV
    {
        public string Skey
        { get; set; }

        public string IV
        { get; set; }

        public string APPuid
        { get; set; }

        public string bfAPPuid
        { get; set; }

        public int ClientID
        { get; set; }

        public string ReturnValue
        { get; set; }

        public string ReturnMessage
        { get; set; }
    }

    class beanfunApp
    {
        private static string currentVersion = System.Reflection.Assembly.GetExecutingAssembly().GetName().Version.ToString();

        public static void Main(string[] args)
        {
            try
            {
                if (args.Length == 0)
                {
                    Console.WriteLine("No Parameter");
                }
                else
                {
                    string[] array = Encoding.UTF8.GetString(Convert.FromBase64String(args[0].Substring(0, args[0].Length - 1).Replace("bfap://", ""))).Split('&');
                    Console.WriteLine("Encoded Parameter");
                    Console.WriteLine($"type:{array[0]},token:{array[1]}");
                    string text = string.Empty;
                    NetworkInterface[] allNetworkInterfaces = NetworkInterface.GetAllNetworkInterfaces();
                    foreach (NetworkInterface networkInterface in allNetworkInterfaces)
                    {
                        if (networkInterface.NetworkInterfaceType == NetworkInterfaceType.Ethernet)
                        {
                            text = networkInterface.GetPhysicalAddress().ToString();
                            break;
                        }
                    }
                    Console.WriteLine($"Get Computer MAC:{text},DeviceName:{Environment.MachineName}");
                    string text2 = array[0];
                    switch (text2)
                    {
                        default:
                            if (text2 == "5")
                            {
                                Console.WriteLine("Request Login");
                                SKIV sKIV = JsonConvert.DeserializeObject<SKIV>(HttpPost(APIUrl.GetSk, "=" + array[1], "2"));
                                Console.WriteLine("Geting Key");
                                if (sKIV.ReturnValue.Equals("1"))
                                {
                                    Console.WriteLine("Geted Key");
                                    Console.WriteLine(HttpPost(APIUrl.CreLl, string.Format("key={0}&data={1}", sKIV.Skey, EncryptAES256("{" + $"bfAPPuid:\"{sKIV.bfAPPuid}\",APPuid:\"{sKIV.APPuid}\",LoginToken:\"{array[1]}\",DeviceName:\"{Environment.MachineName} - 繽放(V{currentVersion})\",MAC:\"{text}\"" + "}", sKIV.Skey, sKIV.IV)), "2"));
                                }
                            }
                            break;
                        case "0":
                            {
                                Console.WriteLine("Register Computer");
                                SKIV sKIV = JsonConvert.DeserializeObject<SKIV>(HttpPost(APIUrl.GetSk, "=" + array[1], "0"));
                                Console.WriteLine("Geting Key");
                                if (sKIV.ReturnValue.Equals("1"))
                                {
                                    Console.WriteLine("Geted Key");
                                    Console.WriteLine("Post CheckRegistration");
                                    Response response2 = JsonConvert.DeserializeObject<Response>(HttpPost(APIUrl.GetRc, string.Format("key={0}&data={1}", sKIV.Skey, EncryptAES256("{" + $"bfAPPuid:\"{sKIV.bfAPPuid}\",APPuid:\"{sKIV.APPuid}\",DeviceName:\"{Environment.MachineName} - 繽放(V{currentVersion})\",MAC:\"{text}\"" + "}", sKIV.Skey, sKIV.IV)), "0"));
                                    if (response2.Result == null)
                                    {
                                        if (!response2.ReturnValue.Equals("1"))
                                        {
                                            Console.WriteLine("Registration Device");
                                            Console.WriteLine(HttpPost(APIUrl.CreRl, string.Format("key={0}&data={1}", sKIV.Skey, EncryptAES256("{" + $"bfAPPuid:\"{sKIV.bfAPPuid}\",APPuid:\"{sKIV.APPuid}\",DeviceName:\"{Environment.MachineName} - 繽放(V{currentVersion})\",MAC:\"{text}\",ClientID:\"{sKIV.ClientID}\"" + "}", sKIV.Skey, sKIV.IV)), "0"));
                                        }
                                        else
                                        {
                                            Console.WriteLine("This device is registered");
                                        }
                                    }
                                    else
                                    {
                                        Console.WriteLine("Cannot connection to check registered");
                                    }
                                }
                                break;
                            }
                        case "1":
                            {
                                Console.WriteLine("Request Login");
                                SKIV sKIV = JsonConvert.DeserializeObject<SKIV>(HttpPost(APIUrl.GetSk, "=" + array[1], "0"));
                                Console.WriteLine("Geting Key");
                                if (sKIV.ReturnValue.Equals("1"))
                                {
                                    Console.WriteLine("Geted Key");
                                    Console.WriteLine(HttpPost(APIUrl.CreLl, string.Format("key={0}&data={1}", sKIV.Skey, EncryptAES256("{" + $"bfAPPuid:\"{sKIV.bfAPPuid}\",APPuid:\"{sKIV.APPuid}\",LoginToken:\"{array[1]}\",DeviceName:\"{Environment.MachineName} - 繽放(V{currentVersion})\",MAC:\"{text}\"" + "}", sKIV.Skey, sKIV.IV)), "0"));
                                }
                                break;
                            }
                        case "2":
                            {
                                Console.WriteLine("Register Computer");
                                SKIV sKIV = JsonConvert.DeserializeObject<SKIV>(HttpPost(APIUrl.GetSk, "=" + array[1], "1"));
                                Console.WriteLine("Geting Key");
                                if (sKIV.ReturnValue.Equals("1"))
                                {
                                    Console.WriteLine("Geted Key");
                                    Console.WriteLine("Post CheckRegistration");
                                    Response response3 = JsonConvert.DeserializeObject<Response>(HttpPost(APIUrl.GetRc, string.Format("key={0}&data={1}", sKIV.Skey, EncryptAES256("{" + $"bfAPPuid:\"{sKIV.bfAPPuid}\",APPuid:\"{sKIV.APPuid}\",DeviceName:\"{Environment.MachineName} - 繽放(V{currentVersion})\",MAC:\"{text}\"" + "}", sKIV.Skey, sKIV.IV)), "1"));
                                    if (response3.Result == null)
                                    {
                                        if (!response3.ReturnValue.Equals("1"))
                                        {
                                            Console.WriteLine("Registration Device");
                                            Console.WriteLine(HttpPost(APIUrl.CreRl, string.Format("key={0}&data={1}", sKIV.Skey, EncryptAES256("{" + $"bfAPPuid:\"{sKIV.bfAPPuid}\",APPuid:\"{sKIV.APPuid}\",DeviceName:\"{Environment.MachineName} - 繽放(V{currentVersion})\",MAC:\"{text}\",ClientID:\"{sKIV.ClientID}\"" + "}", sKIV.Skey, sKIV.IV)), "1"));
                                        }
                                        else
                                        {
                                            Console.WriteLine("This device is registered");
                                        }
                                    }
                                    else
                                    {
                                        Console.WriteLine("Cannot connection to check registered");
                                    }
                                }
                                break;
                            }
                        case "3":
                            {
                                Console.WriteLine("Request Login");
                                SKIV sKIV = JsonConvert.DeserializeObject<SKIV>(HttpPost(APIUrl.GetSk, "=" + array[1], "1"));
                                Console.WriteLine("Geting Key");
                                if (sKIV.ReturnValue.Equals("1"))
                                {
                                    Console.WriteLine("Geted Key");
                                    Console.WriteLine(HttpPost(APIUrl.CreLl, string.Format("key={0}&data={1}", sKIV.Skey, EncryptAES256("{" + $"bfAPPuid:\"{sKIV.bfAPPuid}\",APPuid:\"{sKIV.APPuid}\",LoginToken:\"{array[1]}\",DeviceName:\"{Environment.MachineName} - 繽放(V{currentVersion})\",MAC:\"{text}\"" + "}", sKIV.Skey, sKIV.IV)), "1"));
                                }
                                break;
                            }
                        case "4":
                            {
                                Console.WriteLine("Register Computer");
                                SKIV sKIV = JsonConvert.DeserializeObject<SKIV>(HttpPost(APIUrl.GetSk, "=" + array[1], "2"));
                                Console.WriteLine("Geting Key");
                                if (sKIV.ReturnValue.Equals("1"))
                                {
                                    Console.WriteLine("Geted Key");
                                    Console.WriteLine("Post CheckRegistration");
                                    Response response = JsonConvert.DeserializeObject<Response>(HttpPost(APIUrl.GetRc, string.Format("key={0}&data={1}", sKIV.Skey, EncryptAES256("{" + $"bfAPPuid:\"{sKIV.bfAPPuid}\",APPuid:\"{sKIV.APPuid}\",DeviceName:\"{Environment.MachineName} - 繽放(V{currentVersion})\",MAC:\"{text}\"" + "}", sKIV.Skey, sKIV.IV)), "2"));
                                    if (response.Result == null)
                                    {
                                        if (!response.ReturnValue.Equals("1"))
                                        {
                                            Console.WriteLine("Registration Device");
                                            Console.WriteLine(HttpPost(APIUrl.CreRl, string.Format("key={0}&data={1}", sKIV.Skey, EncryptAES256("{" + $"bfAPPuid:\"{sKIV.bfAPPuid}\",APPuid:\"{sKIV.APPuid}\",DeviceName:\"{Environment.MachineName} - 繽放(V{currentVersion})\",MAC:\"{text}\",ClientID:\"{sKIV.ClientID}\"" + "}", sKIV.Skey, sKIV.IV)), "2"));
                                        }
                                        else
                                        {
                                            Console.WriteLine("This device is registered");
                                        }
                                    }
                                    else
                                    {
                                        Console.WriteLine("Cannot connection to check registered");
                                    }
                                }
                                break;
                            }
                    }
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine(ex.Message);
            }
        }

        public static string EncryptAES256(string source, string key, string iv)
        {
            if (string.IsNullOrEmpty(source) || string.IsNullOrEmpty(key) || string.IsNullOrEmpty(iv))
            {
                return string.Empty;
            }
            byte[] bytes = Encoding.UTF8.GetBytes(source);
            return HttpUtility.UrlEncode(Convert.ToBase64String(new RijndaelManaged
            {
                Key = Encoding.UTF8.GetBytes(key),
                IV = Encoding.UTF8.GetBytes(iv),
                Mode = CipherMode.CBC,
                Padding = PaddingMode.PKCS7
            }.CreateEncryptor().TransformFinalBlock(bytes, 0, bytes.Length)));
        }

        public static string HttpPost(APIUrl PostType, string PostData, string Server)
        {
            string text = string.Empty;
            switch (Server)
            {
                case "0":
                    text = "https://tw.bfapp.beanfun.com/";
                    break;
                case "1":
                    text = "https://alpha-bfapp.beanfun.com/";
                    break;
                case "2":
                    text = "http://localhost:18402/";
                    break;
            }
            switch (PostType)
            {
                case APIUrl.GetSk:
                    text += "api/check/Archaeopteryx0010";
                    break;
                case APIUrl.GetRc:
                    text += "api/check/Archaeopteryx0006";
                    break;
                case APIUrl.CreRl:
                    text += "api/check/Archaeopteryx0007";
                    break;
                case APIUrl.CreLl:
                    text += "api/check/Archaeopteryx0009";
                    break;
            }
            try
            {
                HttpWebRequest httpWebRequest = (HttpWebRequest)WebRequest.Create(text);
                httpWebRequest.Method = "POST";
                httpWebRequest.ContentType = "application/x-www-form-urlencoded";
                byte[] bytes = Encoding.ASCII.GetBytes(PostData);
                httpWebRequest.ContentLength = bytes.Length;
                using (Stream stream = httpWebRequest.GetRequestStream())
                {
                    stream.Write(bytes, 0, bytes.Length);
                    stream.Close();
                }
                using (HttpWebResponse httpWebResponse = (HttpWebResponse)httpWebRequest.GetResponse())
                {
                    return new StreamReader(httpWebResponse.GetResponseStream(), Encoding.UTF8).ReadToEnd();
                }
            }
            catch (Exception ex)
            {
                throw ex;
            }
        }
    }
}
