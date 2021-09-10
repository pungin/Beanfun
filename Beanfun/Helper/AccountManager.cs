/*
 * 開發此功能主要用為多帳號時儲存
 * 以原有加解密寫法為基礎
 * 加上一層wrapper並用Serializable方式儲存資料
 * thanks to Stackoverflow :p
 * http://stackoverflow.com/questions/5869922/c-sharp-encrypt-serialized-file-before-writing-to-disk
 * http://stackoverflow.com/questions/16352879/write-list-of-objects-to-a-file
 * 
 * Date: 2016/3/1
 * Author: 葉家郡 (a.k.a 某數)
 */
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.IO;
using System.Security.Cryptography;
using System.Runtime.Serialization.Formatters.Binary;
using Utility.ModifyRegistry;

namespace Beanfun
{
    [Serializable]
    class AccountRecords
    {
        public List<string> regionList = null, accountList = null, passwdList = null, verifyList = null;
        public List<int> methodList = null;
        public List<bool> autoLoginList = null;
    }

    [Serializable]
    class Records
    {
        public List<string> regionList = null, accountList = null, accountNameList = null, passwdList = null, verifyList = null;
        public List<int> methodList = null;
        public List<bool> autoLoginList = null;

        public static Records Change(object oldRecords) {
            Records res = new Records();
            if (oldRecords is AccountRecords)
            {
                AccountRecords records = (AccountRecords)oldRecords;
                res.regionList = records.regionList;
                res.accountList = records.accountList;
                res.passwdList = records.passwdList;
                res.verifyList = records.verifyList;
                res.methodList = records.methodList;
                res.autoLoginList = records.autoLoginList;
            }
            return res;
        }
    }

    public class AccountManager
    {
        private Records accountRecords = null;
        private string dataPath = System.Environment.GetFolderPath(System.Environment.SpecialFolder.ApplicationData) + "\\Beanfun\\Users.dat";
    

        public bool init()
        {
            return loadRecord();
        }

        #region helper function
        private void accRecInit()
        {
            if (accountRecords == null)
                accountRecords = new Records();

            if (accountRecords.accountList == null) accountRecords.accountList = new List<string>();

            if (accountRecords.regionList == null) accountRecords.regionList = new List<string>();
            if (accountRecords.regionList.Count < accountRecords.accountList.Count)
            {
                for (int i = accountRecords.regionList.Count; i < accountRecords.accountList.Count; i++)
                {
                    accountRecords.regionList.Add("TW");
                }
            }

            if (accountRecords.accountNameList == null) accountRecords.accountNameList = new List<string>();
            if (accountRecords.accountNameList.Count < accountRecords.accountList.Count)
            {
                for (int i = accountRecords.accountNameList.Count; i < accountRecords.accountList.Count; i++)
                {
                    accountRecords.accountNameList.Add("");
                }
            }

            if (accountRecords.passwdList == null) accountRecords.passwdList = new List<string>();
            if (accountRecords.passwdList.Count < accountRecords.accountList.Count)
            {
                for (int i = accountRecords.passwdList.Count; i < accountRecords.accountList.Count; i++)
                {
                    accountRecords.passwdList.Add("");
                }
            }

            if (accountRecords.verifyList == null) accountRecords.verifyList = new List<string>();
            if (accountRecords.verifyList.Count < accountRecords.accountList.Count)
            {
                for (int i = accountRecords.verifyList.Count; i < accountRecords.accountList.Count; i++)
                {
                    accountRecords.verifyList.Add("");
                }
            }

            if (accountRecords.methodList == null) accountRecords.methodList = new List<int>();
            if (accountRecords.methodList.Count < accountRecords.accountList.Count)
            {
                for (int i = accountRecords.methodList.Count; i < accountRecords.accountList.Count; i++)
                {
                    accountRecords.methodList.Add(0);
                }
            }

            if (accountRecords.autoLoginList == null) accountRecords.autoLoginList = new List<bool>();
            if (accountRecords.autoLoginList.Count < accountRecords.accountList.Count)
            {
                for (int i = accountRecords.autoLoginList.Count; i < accountRecords.accountList.Count; i++)
                {
                    accountRecords.autoLoginList.Add(false);
                }
            }
        }

        private bool loadRecord()
        {
            var raw = readRawData();
            if (raw != null)
            {
                byte[] cipher = Convert.FromBase64String(raw);

                using (Stream stream = new MemoryStream(cipher))
                {
                    var bformatter = new System.Runtime.Serialization.Formatters.Binary.BinaryFormatter();

                    object records = bformatter.Deserialize(stream);
                    if (records is AccountRecords)
                    {
                        accountRecords = Records.Change(records);
                    }
                    else
                    {
                        accountRecords = (Records)records;
                    }
                }
            }
            accRecInit();

            return true;
        }

        private bool storeRecord()
        {
            using (var memoryStream = new MemoryStream())
            {
                // Serialize to memory instead of to file
                var formatter = new BinaryFormatter();
                formatter.Serialize(memoryStream, accountRecords);

                // This resets the memory stream position for the following read operation
                memoryStream.Seek(0, SeekOrigin.Begin);

                // Get the bytes
                var bytes = new byte[memoryStream.Length];
                memoryStream.Read(bytes, 0, (int)memoryStream.Length);

                writeRawData(Convert.ToBase64String(bytes));
            }

            return true;
        }
        #endregion

        #region rawdata IO
        /*
         * read ciphertext from File
         * decrypt it and return
         */
        private string readRawData()
        {
            try
            {
                if (File.Exists(dataPath))
                {
                    try
                    {
                        Byte[] cipher = File.ReadAllBytes(dataPath);
                        ModifyRegistry myRegistry = new ModifyRegistry();
                        myRegistry.BaseRegistryKey = Microsoft.Win32.Registry.CurrentUser;
                        string entropy = myRegistry.Read("Entropy");
                        byte[] plaintext = ProtectedData.Unprotect(cipher, Encoding.UTF8.GetBytes(entropy), DataProtectionScope.CurrentUser);
                        return System.Text.Encoding.UTF8.GetString(plaintext);
                    }
                    catch
                    {
                        File.Delete(dataPath);
                    }
                }

                return null;
            }
            catch { return null; }
        }

        /*
         * encrypt plaintext and store to File
         * and save key in Program Setting
         */
        private void writeRawData(string plaintext)
        {
            using (BinaryWriter writer = new BinaryWriter(File.Open(dataPath, FileMode.Create)))
            {
                // Create random entropy of 8 characters.
                var chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
                var random = new Random();
                string entropy = new string(Enumerable.Repeat(chars, 8).Select(s => s[random.Next(s.Length)]).ToArray());
                
                ModifyRegistry myRegistry = new ModifyRegistry();
                myRegistry.BaseRegistryKey = Microsoft.Win32.Registry.CurrentUser;
                myRegistry.Write("Entropy", entropy);

                writer.Write(ciphertext(plaintext, entropy));
            }
        }

        private byte[] ciphertext(string plaintext, string key)
        {
            byte[] plainByte = Encoding.UTF8.GetBytes(plaintext);
            byte[] entropy = Encoding.UTF8.GetBytes(key);
            return ProtectedData.Protect(plainByte, entropy, DataProtectionScope.CurrentUser);
        }
        #endregion

        #region Interface
        public bool addAccount(string region, string account, string name, string password, string verify, int method, bool autoLogin)
        {
            return addAccount(-1, region, account, name, password, verify, method, autoLogin);
        }

        public bool addAccount(int index, string region, string account, string name, string password, string verify, int method, bool autoLogin)
        {
            bool isExists = false;
            List<int> regionIndex = new List<int>();
            for ( int i = 0 ; i < accountRecords.accountList.Count ; ++i )
            {
                if (region != accountRecords.regionList[i])
                {
                    continue;
                }
                if (account == accountRecords.accountList[i])
                {
                    if (index > -1 && regionIndex.Count != index)
                    {
                        removeAccount(region, account);
                        i--;
                        continue;
                    }
                    accountRecords.accountNameList[i] = name;
                    accountRecords.passwdList[i] = password;
                    accountRecords.verifyList[i] = verify;
                    accountRecords.methodList[i] = method;
                    accountRecords.autoLoginList[i] = autoLogin;
                    isExists = true;
                    break;
                }
                regionIndex.Add(i);
            }

            if (!isExists)
            {
                if (index < 0 || regionIndex.Count <= index)
                {
                    accountRecords.regionList.Add(region);
                    accountRecords.accountList.Add(account);
                    accountRecords.accountNameList.Add(name);
                    accountRecords.passwdList.Add(password);
                    accountRecords.verifyList.Add(verify);
                    accountRecords.methodList.Add(method);
                    accountRecords.autoLoginList.Add(autoLogin);
                }
                else
                {
                    index = regionIndex[index];
                    accountRecords.regionList.Insert(index, region);
                    accountRecords.accountList.Insert(index, account);
                    accountRecords.accountNameList.Insert(index, name);
                    accountRecords.passwdList.Insert(index, password);
                    accountRecords.verifyList.Insert(index, verify);
                    accountRecords.methodList.Insert(index, method);
                    accountRecords.autoLoginList.Insert(index, autoLogin);
                }
            }

            storeRecord();

            return true;
        }

        public string getNameByAccount(string region, string account)
        {
            for (int i = 0; i < accountRecords.accountList.Count; ++i)
            {
                if (account == accountRecords.accountList[i] && region == accountRecords.regionList[i])
                {
                    return accountRecords.accountNameList[i];
                }
            }
            return null;
        }

        public string getPasswordByAccount(string region, string account)
        {
            for (int i = 0; i < accountRecords.accountList.Count; ++i)
            {
                if (account == accountRecords.accountList[i] && region == accountRecords.regionList[i])
                {
                    return accountRecords.passwdList[i];
                }
            }
            return null;
        }

        public string getVerifyByAccount(string region, string account)
        {
            for (int i = 0; i < accountRecords.accountList.Count; ++i)
            {
                if (account == accountRecords.accountList[i] && region == accountRecords.regionList[i])
                {
                    return accountRecords.verifyList[i];
                }
            }

            return null;
        }

        public int getMethodByAccount(string region, string account)
        {
            for (int i = 0; i < accountRecords.accountList.Count; ++i)
            {
                if (account == accountRecords.accountList[i] && region == accountRecords.regionList[i])
                {
                    return accountRecords.methodList[i];
                }
            }

            return -1;
        }

        public bool getAutoLoginByAccount(string region, string account)
        {
            for (int i = 0; i < accountRecords.accountList.Count; ++i)
            {
                if (account == accountRecords.accountList[i] && region == accountRecords.regionList[i])
                {
                    return accountRecords.autoLoginList[i];
                }
            }

            return false;
        }

        public bool removeAccount(string region, string account)
        {
            for (int i = 0; i < accountRecords.accountList.Count; ++i)
            {
                if (account == accountRecords.accountList[i] && region == accountRecords.regionList[i])
                {
                    accountRecords.regionList.RemoveAt(i);
                    accountRecords.accountList.RemoveAt(i);
                    accountRecords.accountNameList.RemoveAt(i);
                    accountRecords.passwdList.RemoveAt(i);
                    accountRecords.verifyList.RemoveAt(i);
                    accountRecords.methodList.RemoveAt(i);
                    accountRecords.autoLoginList.RemoveAt(i);

                    storeRecord();
                    return true;
                }
            }

            return false;
        }

        public string[] getAccountList()
        {
            return accountRecords.accountList.ToArray();
        }

        public string[] getAccountList(string region)
        {
            List<string> accList = new List<string>();
            for (int i = 0; i < accountRecords.accountList.Count; ++i)
            {
                if (region == accountRecords.regionList[i])
                {
                    accList.Add(accountRecords.accountList[i]);
                }
            }
            return accList.ToArray();
        }

        public bool importRecord(string raw)
        {
            try
            {
                byte[] cipher = Convert.FromBase64String(raw);

                using (Stream stream = new MemoryStream(cipher))
                {
                    var bformatter = new System.Runtime.Serialization.Formatters.Binary.BinaryFormatter();

                    object records = bformatter.Deserialize(stream);
                    if (records is AccountRecords)
                    {
                        accountRecords = Records.Change(records);
                    }
                    else
                    {
                        accountRecords = (Records)records;
                    }
                }
                accRecInit();
                storeRecord();
            }
            catch
            {
                return false;
            }

            return true;
        }

        public string exportRecord()
        {
            using (var memoryStream = new MemoryStream())
            {
                // Serialize to memory instead of to file
                var formatter = new BinaryFormatter();
                formatter.Serialize(memoryStream, accountRecords);

                // This resets the memory stream position for the following read operation
                memoryStream.Seek(0, SeekOrigin.Begin);

                // Get the bytes
                var bytes = new byte[memoryStream.Length];
                memoryStream.Read(bytes, 0, (int)memoryStream.Length);

                return Convert.ToBase64String(bytes);
            }
        }
        #endregion
    }
}
