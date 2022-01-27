using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Windows;

namespace Beanfun
{
    /// <summary>
    /// CoreCalculator.xaml 的交互逻辑
    /// </summary>
    public partial class CoreCalculator : Window
    {
        public ObservableCollection<string> MustSkills { get; private set; } = new ObservableCollection<string>();
        private ObservableCollection<string> _mainSkillSource = new ObservableCollection<string>();
        private int _useOtherSkillCount = 0;
        public ObservableCollection<string> MainSkillSource
        {
            get
            {
                if (_mainSkillSource.Count != MustSkills.Count + _useOtherSkillCount + 1)
                {
                    _mainSkillSource = new ObservableCollection<string>(MustSkills);
                    for(int i = 0; i <= _useOtherSkillCount; i++)
                    {
                        _mainSkillSource.Add(string.Format("{0}{1}", TryFindResource("Others"), i + 1));
                    }
                }
                return _mainSkillSource;
            }
        }
        private ObservableCollection<string> _secondarySkillSource = new ObservableCollection<string>();
        public ObservableCollection<string> SecondarySkillSource
        {
            get
            {
                if(_secondarySkillSource.Count != MustSkills.Count + 1)
                {
                    _secondarySkillSource = new ObservableCollection<string>(MustSkills);
                    _secondarySkillSource.Add(TryFindResource("Others") as string);
                }
                return _secondarySkillSource;
            }
        }
        public ObservableCollection<CoreItem> CoreItems { get; private set; } = new ObservableCollection<CoreItem>();
        public CoreCalculator()
        {
            InitializeComponent();
            DataContext = this;
        }

        private void btn_AddMustSkill_Click(object sender, RoutedEventArgs e)
        {
            if (t_SkillName.Text == "")
            {
                this.errorMessage(TryFindResource("SkillNameIsEmpty") as string);
                return;
            }
            if (MustSkills.Contains(t_SkillName.Text))
            {
                this.errorMessage(TryFindResource("SkillNameIsRepeat") as string);
                return;
            }
            MustSkills.Add(t_SkillName.Text);
            c_Skill1.ItemsSource = MainSkillSource;
            c_Skill2.ItemsSource = SecondarySkillSource;
            c_Skill3.ItemsSource = SecondarySkillSource;
            btn_Calculator.Content = string.Format(TryFindResource("CaculatorCore") as string, this.mustCoreCount(), MustSkills.Count);
            t_SkillName.Text = "";
        }

        private void btn_AddCore_Click(object sender, RoutedEventArgs e)
        {
            if (c_Skill1.Text == "" || c_Skill2.Text == "" || c_Skill3.Text == "")
            {
                this.errorMessage(TryFindResource("CoreSkillNameIsEmpty") as string);
                return;
            }
            if (c_Skill1.Text == c_Skill2.Text || c_Skill1.Text == c_Skill3.Text || c_Skill2.Text == c_Skill3.Text && c_Skill2.Text != TryFindResource("Others") as string) 
            {
                this.errorMessage(TryFindResource("CoreSkillNameIsRepeat") as string);
                return;
            }
            CoreItem item = new CoreItem(c_Skill1.Text, c_Skill2.Text, c_Skill3.Text);
            if (CoreItems.Contains(item))
            {
                this.errorMessage(TryFindResource("CoreIsRepeat") as string);
                return;
            }
            CoreItems.Add(item);
            if (item.skill1 == string.Format("{0}{1}", TryFindResource("Others"), _useOtherSkillCount + 1))
            {
                _useOtherSkillCount++;
                c_Skill1.ItemsSource = MainSkillSource;
            }
        }

        private void btn_DeleteCore_Click(object sender, RoutedEventArgs e)
        {
            if(l_Cores.SelectedItem is CoreItem)
            {
                CoreItems.Remove((CoreItem)l_Cores.SelectedItem);
            }
        }

        private void btn_Calculator_Click(object sender, RoutedEventArgs e)
        {
            btn_Calculator.IsEnabled = false;
            int must_count = this.mustCoreCount();
            List<List<CoreItem>> result = new List<List<CoreItem>>();
            int size = CoreItems.Count;
            bool[] zero = new bool[size];
            for(int i = 0; i < size; i++)
            {
                zero[i] = i < must_count;
            }
            for(; ; )
            {
                List<CoreItem> sub = new List<CoreItem>();
                Dictionary<string, bool> keys = new Dictionary<string, bool>();
                int index = -1;
                bool per1 = false;
                int leftCount = -1;
                for(int i = 0; i < size; i++)
                {
                    if (zero[i])
                    {
                        CoreItem item = CoreItems[i];
                        if (!keys.ContainsKey(item.skill1))
                        {
                            keys[item.skill1] = true;
                            sub.Add(item);
                        }
                    }
                    if (index == -1)
                    {
                        if(per1 && !zero[i])
                        {
                            zero[i] = true;
                            index = i;
                        }
                        else
                        {
                            per1 = zero[i];
                            if (per1) leftCount++;
                        }
                    }
                }
                for(int i = 0; i < index; i++)
                {
                    zero[i] = i < leftCount;
                }
                if (sub.Count == must_count)
                {
                    Dictionary<string, int> temp = new Dictionary<string, int>();
                    foreach(string skill in MustSkills)
                    {
                        foreach(CoreItem item in sub)
                        {
                            if(skill==item.skill1 || skill==item.skill2 || skill == item.skill3)
                            {
                                temp[skill] = temp.ContainsKey(skill) ? temp[skill] + 1 : 1;
                            }
                        }
                    }
                    bool isPerfect = true;
                    foreach (string skill in MustSkills)
                    {
                        if (!temp.ContainsKey(skill) || temp[skill] < 2)
                        {
                            isPerfect = false;
                            break;
                        }
                    }
                    if (isPerfect) result.Add(sub);
                }

                if (index == -1) break;
            }
            if (result.Count <= 0)
            {
                t_Result.Text = TryFindResource("NotFindPerfectCore") as string + "\r\nBy:LinTx";
            }
            else
            {
                string r = "";
                for (int i = 0; i < result.Count; i++)
                {
                    List<CoreItem> items = result[i];
                    r += string.Format(TryFindResource("CoreGroup") as string, i + 1) + "\r\n";
                    foreach(CoreItem item in items)
                    {
                        r = r + item.ToString() + "\r\n";
                    }
                    r = r + "\r\n";
                }
                r = r + "By:LinTx";
                t_Result.Text = r;
            }
            btn_Calculator.IsEnabled = true;
        }

        private int mustCoreCount()
        {
            int count = (int)Math.Ceiling((double)MustSkills.Count * 2 / 3);
            if (count < 2) count = 2;
            return count;
        }

        private void errorMessage(string message)
        {
            MessageBox.Show(message, TryFindResource("SystemInfo") as string, MessageBoxButton.OK, MessageBoxImage.Error);
        }

        private void btn_DeleteMustSkill_Click(object sender, RoutedEventArgs e)
        {
            if (l_MustSkills.SelectedItem is string)
            {
                MustSkills.Remove((string)l_MustSkills.SelectedItem);
                c_Skill1.ItemsSource = MainSkillSource;
                c_Skill2.ItemsSource = SecondarySkillSource;
                c_Skill3.ItemsSource = SecondarySkillSource;
                btn_Calculator.Content = string.Format(TryFindResource("CaculatorCore") as string, this.mustCoreCount(), MustSkills.Count);
            }
            
        }
    }

    public class CoreItem : IEquatable<CoreItem>
    {
        public string skill1 { get; }
        public string skill2 { get; }
        public string skill3 { get; }
        public CoreItem(string skill1,string skill2,string skill3)
        {
            this.skill1 = skill1;
            this.skill2 = skill2;
            this.skill3 = skill3;
        }
        public override int GetHashCode()
        {
            int hash1 = skill1.GetHashCode();
            int hash2 = skill2.GetHashCode();
            int hash3 = skill3.GetHashCode();
            return hash1 + hash2 + hash3;
        }

        public override string ToString()
        {
            return string.Format("{0}({1})/{2}/{3}", this.skill1, Application.Current.TryFindResource("Main"), this.skill2, this.skill3);
        }

        public bool Equals(CoreItem other)
        {
            return this.skill1 == other.skill1 && (this.skill2 == other.skill2 && this.skill3 == other.skill3 || this.skill2 == other.skill3 && this.skill3 == other.skill2);
        }

        public string[] Skills
        {
            get
            {
                return new string[] { this.skill1, this.skill2, this.skill3 };
            }
        }
    }
}
