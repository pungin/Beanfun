import { describe, expect, it } from 'vitest'

// `convert-lang.mjs` is a Node ESM script (no .d.ts), so we declare the
// shape we rely on locally and use a dynamic import to bypass the
// implicit-any complaint without touching tsconfig.
type ConvertLangModule = {
  parseXamlStrings: (xaml: string) => Record<string, string>
  LOCALE_FILE_MAP: ReadonlyArray<{ source: string; target: string }>
}

const importConvertLang = async (): Promise<ConvertLangModule> => {
  const mod = (await import('../../../scripts/convert-lang.mjs')) as unknown
  return mod as ConvertLangModule
}

describe('parseXamlStrings', () => {
  it('extracts all <system:String x:Key="…"> entries as a flat KV object', async () => {
    const { parseXamlStrings } = await importConvertLang()
    const xaml = `<?xml version="1.0" encoding="utf-8" ?>
<ResourceDictionary
  xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
  xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
  xmlns:system="clr-namespace:System;assembly=mscorlib"
>
  <system:String x:Key="AppName">繽放</system:String>
  <system:String x:Key="Login">登入</system:String>
  <system:String x:Key="Cancel">取消</system:String>
</ResourceDictionary>`

    const out = parseXamlStrings(xaml)
    expect(out).toEqual({
      AppName: '繽放',
      Login: '登入',
      Cancel: '取消',
    })
  })

  it('preserves insertion order so diffs stay reviewable', async () => {
    const { parseXamlStrings } = await importConvertLang()
    const xaml = `<ResourceDictionary
  xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
  xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
  xmlns:system="clr-namespace:System;assembly=mscorlib"
>
  <system:String x:Key="C">3</system:String>
  <system:String x:Key="A">1</system:String>
  <system:String x:Key="B">2</system:String>
</ResourceDictionary>`

    expect(Object.keys(parseXamlStrings(xaml))).toEqual(['C', 'A', 'B'])
  })

  it('preserves {0}/{1} placeholders verbatim (vue-i18n list-mode compatible)', async () => {
    const { parseXamlStrings } = await importConvertLang()
    const xaml = `<ResourceDictionary
  xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
  xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
  xmlns:system="clr-namespace:System;assembly=mscorlib"
>
  <system:String x:Key="GashRemain">樂豆: {0} 點</system:String>
  <system:String x:Key="MsgDeleteAccount">即將移除帳號「{0}」，是否確認？</system:String>
  <system:String x:Key="FeedbackText">軟體版本: {0}%0d反饋訊息:%0d</system:String>
</ResourceDictionary>`

    const out = parseXamlStrings(xaml)
    expect(out.GashRemain).toBe('樂豆: {0} 點')
    expect(out.MsgDeleteAccount).toBe('即將移除帳號「{0}」，是否確認？')
    expect(out.FeedbackText).toBe('軟體版本: {0}%0d反饋訊息:%0d')
  })

  it('decodes XML entities (&lt; &gt; &quot;) inside string content', async () => {
    const { parseXamlStrings } = await importConvertLang()
    const xaml = `<ResourceDictionary
  xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
  xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
  xmlns:system="clr-namespace:System;assembly=mscorlib"
>
  <system:String x:Key="AboutText">&lt;R&gt;本程式&lt;B&gt;不是&lt;/B&gt;官方&lt;/R&gt;</system:String>
  <system:String x:Key="Quoted">say &quot;hi&quot;</system:String>
</ResourceDictionary>`

    const out = parseXamlStrings(xaml)
    expect(out.AboutText).toBe('<R>本程式<B>不是</B>官方</R>')
    expect(out.Quoted).toBe('say "hi"')
  })

  it('skips non-string resources (Geometry, TextBlock, etc.)', async () => {
    const { parseXamlStrings } = await importConvertLang()
    const xaml = `<ResourceDictionary
  xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
  xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
  xmlns:system="clr-namespace:System;assembly=mscorlib"
>
  <Geometry x:Key="LogoName">M0,0L1,1Z</Geometry>
  <system:String x:Key="AppName">繽放</system:String>
  <TextBlock x:Key="AutoPasteTip" TextWrapping="Wrap">
    <Run Text="hint" />
  </TextBlock>
  <system:String x:Key="Login">登入</system:String>
</ResourceDictionary>`

    expect(parseXamlStrings(xaml)).toEqual({
      AppName: '繽放',
      Login: '登入',
    })
  })

  it('returns an empty object when the dictionary has no system:String entries', async () => {
    const { parseXamlStrings } = await importConvertLang()
    const xaml = `<ResourceDictionary
  xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
  xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
>
  <Geometry x:Key="OnlyShape">M0,0Z</Geometry>
</ResourceDictionary>`

    expect(parseXamlStrings(xaml)).toEqual({})
  })

  it('throws a clear error when the XAML root is not <ResourceDictionary>', async () => {
    const { parseXamlStrings } = await importConvertLang()
    const xaml = `<NotADictionary><system:String x:Key="K">V</system:String></NotADictionary>`
    expect(() => parseXamlStrings(xaml)).toThrow(/ResourceDictionary/)
  })
})

describe('LOCALE_FILE_MAP', () => {
  it('maps the three legacy WPF XAML files to canonical vue-i18n locale codes', async () => {
    const { LOCALE_FILE_MAP } = await importConvertLang()
    expect(LOCALE_FILE_MAP).toEqual([
      { source: 'zh.xaml', target: 'zh-TW.json' },
      { source: 'zh-Hans.xaml', target: 'zh-CN.json' },
      { source: 'en.xaml', target: 'en-US.json' },
    ])
  })
})
