using Amemiya.Net;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Threading;
using System.Windows;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// DownloadProgressBar.xaml 的交互逻辑
    /// </summary>
    public partial class DownloadProgressBar : Window
    {
        private WebClient DownloaderClient;
        private List<string> TaskFiles = new List<string>();
        public int TaskFileNum = 0;
        public int DownloadedFileNum = 0;

        private bool IsShowFileName;
        private string WndTitle;
        string BaseDir = $"{System.Environment.CurrentDirectory}\\";

        public DownloadProgressBar(List<string> taskFiles, string title = "正在下載...", string dir = null, bool isShowFileName = true)
        {
            InitializeComponent();
            if (!App.IsWin10) SourceChord.FluentWPF.AcrylicWindow.SetTintOpacity(this, 1.0);
            DownloaderClient = new WebClientEx();
            WndTitle = title;
            if (dir != null) BaseDir = dir;
            IsShowFileName = isShowFileName;
            DownloaderClient.DownloadProgressChanged += DownloadProgressChanged;
            DownloaderClient.DownloadFileCompleted += DownloadFileCompleted;
            TaskFiles = taskFiles;
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            Download();
        }

        private void Window_Closing(object sender, CancelEventArgs e)
        {
            DownloaderClient.CancelAsync();
        }

        private void DownloadProgressChanged(object sender, DownloadProgressChangedEventArgs e)
        {
            Dispatcher.Invoke(new Action(() =>
                {
                    pbDownload.Value = e.ProgressPercentage;
                    lblDownload.Content = e.ProgressPercentage.ToString() + " %";
                }
            ));
        }

        private void DownloadFileCompleted(object sender, AsyncCompletedEventArgs e)
        {
            if (e.Cancelled)
            {
                return;
            }
            DownloadedFileNum++;
            if (DownloadedFileNum == TaskFileNum)
            {
                this.Close();
                return;
            }
            if (DownloaderClient != null && DownloaderClient.IsBusy)
            {
                DownloaderClient.CancelAsync();
            }
            if (DownloadedFileNum < TaskFileNum)
            {
                Download(TaskFiles[DownloadedFileNum]);
            }
        }

        private void Download()
        {
            if (TaskFiles != null)
            {
                TaskFileNum = TaskFiles.Count;
                if (TaskFileNum > 0)
                {
                    string url = TaskFiles[0];
                    Download(url);
                }
            }
        }

        private void Download(string url)
        {
            if (!Directory.Exists(BaseDir))
            {
                Directory.CreateDirectory(BaseDir);
            }
            string fileName = url.Substring(url.LastIndexOf("/") + 1);
            string path = $"{BaseDir}{fileName}";
            if (File.Exists(path))
            {
                File.Delete(path);
            }
            if (IsShowFileName)
            {
                lblFileName.Visibility = Visibility.Visible;
                lblFileName.Content = fileName;
            }
            else
            {
                lblFileName.Visibility = Visibility.Collapsed;
            }
            this.Title = $"{ WndTitle }{ (TaskFileNum > 1 ? ($"({ DownloadedFileNum + 1 }/{ TaskFileNum })") : "") }";
            DownloaderClient.DownloadFileAsync(new Uri(url), path);
        }
    }
}
