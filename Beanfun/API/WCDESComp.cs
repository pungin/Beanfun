using System;
using System.Security.Cryptography;
using System.Text;

namespace Beanfun
{
    class WCDESComp
    {
        public static string EncryStrHex(string str, string key)
        {
            try
            {
                DESCryptoServiceProvider des = new DESCryptoServiceProvider();
                des.Mode = CipherMode.ECB;
                des.Padding = PaddingMode.None;
                des.Key = Encoding.ASCII.GetBytes(key);
                byte[] byteArray = Encoding.ASCII.GetBytes(str);
                ICryptoTransform desencrypt = des.CreateEncryptor();
                byte[] byteOUT = desencrypt.TransformFinalBlock(byteArray, 0, byteArray.Length);
                return BitConverter.ToString(byteOUT).Replace("-", "");
            }
            catch (Exception e)
            {
                Console.WriteLine("EncryptDESError:" + e.Message + "\n" + e.StackTrace);
                return null;
            }
        }

        public static string DecryStrHex(string hexString, string key)
        {
            try
            {
                DESCryptoServiceProvider des = new DESCryptoServiceProvider();
                des.Mode = CipherMode.ECB;
                des.Padding = PaddingMode.None;
                des.Key = Encoding.ASCII.GetBytes(key);
                byte[] byteOUT = new byte[hexString.Length / 2];
                for (int i = 0; i < hexString.Length; i += 2)
                {
                    byteOUT[i / 2] = Convert.ToByte(hexString.Substring(i, 2), 16);
                }
                ICryptoTransform desdecrypt = des.CreateDecryptor();
                return Encoding.ASCII.GetString(desdecrypt.TransformFinalBlock(byteOUT, 0, byteOUT.Length));
            }
            catch (Exception e)
            {
                Console.WriteLine("DecryptDESError:" + e.Message + "\n" + e.StackTrace);
                return null;
            }
        }
    }
}
