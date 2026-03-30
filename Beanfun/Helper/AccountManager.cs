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
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using Newtonsoft.Json;
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

        public static Records Change(object oldRecords)
        {
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
        private static readonly log4net.ILog log = log4net.LogManager.GetLogger(typeof(AccountManager));

        private Records accountRecords = null;
        private string dataPath = System.Environment.GetFolderPath(System.Environment.SpecialFolder.ApplicationData) + "\\Beanfun\\Users.dat";

        public bool init()
        {
            return loadRecord();
        }

        #region helper function
        private void accRecInit()
        {
            if (accountRecords == null) accountRecords = new Records();

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
                try
                {
                    // 嘗試以新版 JSON 格式讀取資料
                    accountRecords = JsonConvert.DeserializeObject<Records>(raw);
                }
                catch
                {
                    accountRecords = null;
                    // 解析失敗時，自動視為舊版 BinaryFormatter 格式並嘗試進行無縫轉換
                    TryAutoMigrateLegacyData(raw);
                }
            }
            accRecInit();

            return true;
        }

        private bool storeRecord()
        {
            string json = JsonConvert.SerializeObject(accountRecords);
            writeRawData(json);
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
                        return Encoding.UTF8.GetString(plaintext);
                    }
                    catch
                    {
                        File.Delete(dataPath);
                    }
                }

                return null;
            }
            catch
            {
                return null;
            }
        }

        /*
         * encrypt plaintext and store to File
         * and save key in Program Setting
         */
        private void writeRawData(string plaintext)
        {
            using (BinaryWriter writer = new BinaryWriter(File.Open(dataPath, FileMode.Create)))
            {
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
            for (int i = 0; i < accountRecords.accountList.Count; ++i)
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
                accountRecords = JsonConvert.DeserializeObject<Records>(raw);
                accRecInit();
                storeRecord();
                return true;
            }
            catch
            {
                // 匯入失敗時，嘗試將其視為舊版格式進行轉換
                return TryAutoMigrateLegacyData(raw);
            }
        }

        public string exportRecord()
        {
            return JsonConvert.SerializeObject(accountRecords);
        }
        #endregion

        #region Legacy format migration
        // Fix #182: 實作內建的舊版資料自動升級機制，取代原先會導致 404 的外部轉換工具
        // TODO: 此升級機制僅為過渡用途。建議於發布幾個版本後，確認多數活躍玩家皆已轉換至 JSON 格式時，將此方法徹底移除。
        private bool TryAutoMigrateLegacyData(string raw)
        {
            try
            {
                byte[] cipher = Convert.FromBase64String(raw);
                using (var stream = new MemoryStream(cipher))
                {
                    // 忽略編譯器針對 BinaryFormatter 的安全性警告
                    // 注意：此類別極度不安全，僅限於此處讀取舊版資料使用，新代碼嚴禁使用！
#pragma warning disable SYSLIB0011
                    var bformatter = new System.Runtime.Serialization.Formatters.Binary.BinaryFormatter();
                    object oldRecords = bformatter.Deserialize(stream);
#pragma warning restore SYSLIB0011

                    if (oldRecords != null)
                    {
                        // 透過 JSON 序列化作為中介，避免類別轉型 (Casting) 發生例外狀況
                        string tempJson = JsonConvert.SerializeObject(oldRecords);
                        accountRecords = JsonConvert.DeserializeObject<Records>(tempJson);

                        if (accountRecords != null)
                        {
                            accRecInit();
                            storeRecord(); // 立即將轉換後的資料以最新 JSON 格式寫入，覆寫舊檔

                            log.Info("偵測到舊版帳號資料，已成功自動升級至 JSON 格式。");
                            System.Windows.MessageBox.Show(
                                "系統已成功將您的舊版帳號資料自動升級至新格式！\n您現在可以正常使用所有帳號。",
                                "資料轉換成功",
                                System.Windows.MessageBoxButton.OK,
                                System.Windows.MessageBoxImage.Information
                            );
                            return true;
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                // 若因 .NET 版本限制或資料損毀導致轉換失敗，則記錄錯誤，並讓 accRecInit 建立新的空白紀錄
                log.Error($"自動轉換舊版資料失敗: {ex.Message}");
            }

            return false;
        }
        #endregion
    }
}