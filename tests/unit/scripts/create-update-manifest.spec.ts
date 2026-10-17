import { describe, expect, it } from 'vitest'

type ManifestOptions = {
  version: string
  tag: string
  repository: string
  sourceCommit: string
  publishedAt: string
  notes: string
}

type Manifest = {
  schema_version: number
  version: string
  tag: string
  source_commit: string
  notes: string
  pub_date: string
  platforms: Record<string, { url: string; sha256: string; size: number }>
}

const { createUpdateManifest } = (await import('../../../scripts/create-update-manifest.mjs')) as {
  createUpdateManifest: (
    options: ManifestOptions,
    readArtifact: (path: string) => Buffer,
  ) => Manifest
}

const options: ManifestOptions = {
  version: '6.3.0',
  tag: 'v6.3.0.2610180317',
  repository: 'pungin/Beanfun',
  sourceCommit: '0123456789abcdef0123456789abcdef01234567',
  publishedAt: '2026-10-18T03:17:00.000Z',
  notes: 'An update with multiple lines.\n保留更新說明。',
}

/** Minimal synthetic PE headers; these fixtures are never executed. */
function executableFixture(fill: number) {
  const executable = Buffer.alloc(128, fill)
  executable.write('MZ', 0, 'ascii')
  executable.writeUInt32LE(64, 0x3c)
  executable.writeUInt32LE(0x00004550, 64)
  return executable
}

const portable = executableFixture(1)
const installer = executableFixture(2)

function reader(overrides: Record<string, Buffer | null> = {}) {
  const files: Record<string, Buffer | null> = {
    'Beanfun.exe': portable,
    'bundle/nsis/Beanfun_6.3.0_x64-setup.exe': installer,
    ...overrides,
  }
  return (path: string) => {
    const contents = files[path]
    if (!contents) throw new Error(`Missing artifact: ${path}`)
    return contents
  }
}

describe('createUpdateManifest', () => {
  it('binds both exact assets to their hashes, lengths, release tag and build source', () => {
    expect(createUpdateManifest(options, reader())).toEqual({
      schema_version: 1,
      version: '6.3.0',
      tag: options.tag,
      source_commit: options.sourceCommit,
      notes: options.notes,
      pub_date: options.publishedAt,
      platforms: {
        'windows-x86_64': {
          url: 'https://github.com/pungin/Beanfun/releases/download/v6.3.0.2610180317/Beanfun_6.3.0_x64-setup.exe',
          sha256: '772b08899be8d709e9991e8156e3c3fc5b15ec2749927c0901f46f87bd61bed5',
          size: 128,
        },
        'windows-x86_64-portable': {
          url: 'https://github.com/pungin/Beanfun/releases/download/v6.3.0.2610180317/Beanfun.exe',
          sha256: 'a3b599da759a51f5881ce1a9739f56d2750f0ac4a611ee8c47585a5599b2ff1d',
          size: 128,
        },
      },
    })
  })

  it.each(['v6.3.0', '6.3.0.2610180317', '6.3.0-beta.1', '06.3.0', '', '../6.3.0'])(
    'rejects an unsupported version: %s',
    (version) => {
      expect(() => createUpdateManifest({ ...options, version }, reader())).toThrow('version')
    },
  )

  it.each(['v6.2.0.2610180317', 'v6.3.0', 'v6.3.0.2610180317/../other', 'v6.3.0.2610180317?x'])(
    'rejects a mismatched or unsafe release tag: %s',
    (tag) => {
      expect(() => createUpdateManifest({ ...options, tag }, reader())).toThrow('tag')
    },
  )

  it.each([
    'https://github.com/pungin/Beanfun',
    '../Beanfun',
    'pungin/..',
    'pungin/Beanfun?x',
    'other/Beanfun',
    'pungin/beanfun',
  ])('rejects an invalid repository: %s', (repository) => {
    expect(() => createUpdateManifest({ ...options, repository }, reader())).toThrow('Repository')
  })

  it.each(['', '0123456', 'G'.repeat(40), 'A'.repeat(40), '../' + 'a'.repeat(37)])(
    'rejects an invalid source commit: %s',
    (sourceCommit) => {
      expect(() => createUpdateManifest({ ...options, sourceCommit }, reader())).toThrow(
        'Source commit',
      )
    },
  )

  it.each(['', '2026-02-30T03:17:00.000Z', '2026-10-18', '2026-10-18T03:17:00+00:00'])(
    'rejects an invalid publication timestamp: %s',
    (publishedAt) => {
      expect(() => createUpdateManifest({ ...options, publishedAt }, reader())).toThrow('date')
    },
  )

  it('accepts a UTC publication timestamp without milliseconds', () => {
    const publishedAt = '2026-10-18T03:17:00Z'
    expect(createUpdateManifest({ ...options, publishedAt }, reader()).pub_date).toBe(publishedAt)
  })

  it.each(['Beanfun.exe', 'bundle/nsis/Beanfun_6.3.0_x64-setup.exe'])(
    'fails when either update target lacks an artifact: %s',
    (path) => {
      expect(() => createUpdateManifest(options, reader({ [path]: null }))).toThrow(
        'Missing artifact',
      )
    },
  )

  it.each([
    Buffer.alloc(0),
    Buffer.from('<html>upstream download failed</html>'),
    Buffer.from('MZ truncated file'),
  ])('rejects an empty executable or a failed download', (contents) => {
    expect(() => createUpdateManifest(options, reader({ 'Beanfun.exe': contents }))).toThrow(
      'Windows executable',
    )
  })

  it.each([0, 63, 125, 0xffffffff])('rejects a PE header outside the executable: %s', (offset) => {
    const contents = Buffer.from(portable)
    contents.writeUInt32LE(offset, 0x3c)
    expect(() => createUpdateManifest(options, reader({ 'Beanfun.exe': contents }))).toThrow(
      'Windows executable',
    )
  })

  it('rejects an invalid PE header', () => {
    const contents = Buffer.from(portable)
    contents.writeUInt32LE(0, 64)
    expect(() => createUpdateManifest(options, reader({ 'Beanfun.exe': contents }))).toThrow(
      'Windows executable',
    )
  })

  it('rejects executables larger than the client download limit before hashing them', () => {
    const contents = Buffer.from(portable)
    Object.defineProperty(contents, 'length', { value: 512 * 1024 * 1024 + 1 })
    expect(() => createUpdateManifest(options, reader({ 'Beanfun.exe': contents }))).toThrow(
      'Windows executable',
    )
  })

  it('hashes the complete executable, including bytes after the PE header', () => {
    const changed = Buffer.from(portable)
    changed[changed.length - 1] ^= 1
    const originalManifest = createUpdateManifest(options, reader())
    const changedManifest = createUpdateManifest(options, reader({ 'Beanfun.exe': changed }))
    expect(changedManifest.platforms['windows-x86_64-portable'].sha256).not.toBe(
      originalManifest.platforms['windows-x86_64-portable'].sha256,
    )
    expect(changedManifest.platforms['windows-x86_64']).toEqual(
      originalManifest.platforms['windows-x86_64'],
    )
  })

  it('rejects a manifest whose UTF-8 bytes exceed the client download limit', () => {
    expect(() =>
      createUpdateManifest({ ...options, notes: '更新'.repeat(180_000) }, reader()),
    ).toThrow('1 MiB')
  })
})
