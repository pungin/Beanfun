#!/usr/bin/env node
// @ts-check

import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const MAX_EXECUTABLE_BYTES = 512 * 1024 * 1024
const MAX_MANIFEST_BYTES = 1024 * 1024

/**
 * @typedef {object} ManifestOptions
 * @property {string} version
 * @property {string} tag
 * @property {string} repository
 * @property {string} sourceCommit
 * @property {string} publishedAt
 * @property {string} notes
 */

/**
 * Create both updater targets from the exact files produced by the release build.
 * The reader is injectable so validation can be exercised without building an installer.
 * @param {ManifestOptions} options
 * @param {(relativePath: string) => Buffer} readArtifact
 */
export function createUpdateManifest(options, readArtifact) {
  const { version, tag, repository, sourceCommit, publishedAt, notes } = options
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
    throw new Error('Release version must use the X.Y.Z format without a build timestamp')
  }
  if (!new RegExp(`^v${version.replaceAll('.', '\\.')}\\.\\d{10}$`).test(tag)) {
    throw new Error('Release tag must match the version and its ten-digit build timestamp')
  }
  if (repository !== 'pungin/Beanfun') {
    throw new Error('Repository must be pungin/Beanfun, matching the trusted release workflow')
  }
  if (!/^[a-f0-9]{40}$/.test(sourceCommit)) {
    throw new Error('Source commit must be a full lowercase Git commit SHA')
  }
  if (
    !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/.test(publishedAt) ||
    !Number.isFinite(Date.parse(publishedAt)) ||
    new Date(publishedAt).toISOString() !==
      (publishedAt.includes('.') ? publishedAt : publishedAt.replace('Z', '.000Z'))
  ) {
    throw new Error('Publication date must be a valid UTC RFC 3339 timestamp')
  }
  if (typeof notes !== 'string') {
    throw new Error('Release notes must be text')
  }

  /** @type {Record<string, {url: string, sha256: string, size: number}>} */
  const platforms = {}
  for (const [target, relativePath] of [
    ['windows-x86_64', `bundle/nsis/Beanfun_${version}_x64-setup.exe`],
    ['windows-x86_64-portable', 'Beanfun.exe'],
  ]) {
    const executable = readArtifact(relativePath)
    if (
      executable.length < 64 ||
      executable.length > MAX_EXECUTABLE_BYTES ||
      executable.subarray(0, 2).toString('ascii') !== 'MZ'
    ) {
      throw new Error(`Missing or invalid Windows executable: ${relativePath}`)
    }
    const peOffset = executable.readUInt32LE(0x3c)
    if (
      peOffset < 64 ||
      peOffset > executable.length - 4 ||
      executable.readUInt32LE(peOffset) !== 0x00004550
    ) {
      throw new Error(`Missing or invalid Windows executable: ${relativePath}`)
    }
    const filename = relativePath.split('/').at(-1)
    platforms[target] = {
      url: `https://github.com/${repository}/releases/download/${tag}/${filename}`,
      sha256: createHash('sha256').update(executable).digest('hex'),
      size: executable.length,
    }
  }
  const manifest = {
    schema_version: 1,
    version,
    tag,
    source_commit: sourceCommit,
    pub_date: publishedAt,
    notes,
    platforms,
  }
  if (Buffer.byteLength(serialiseManifest(manifest)) > MAX_MANIFEST_BYTES) {
    throw new Error('Update manifest exceeds the 1 MiB download limit')
  }
  return manifest
}

/** @param {ReturnType<typeof createUpdateManifest>} manifest */
function serialiseManifest(manifest) {
  return `${JSON.stringify(manifest, null, 2)}\n`
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  try {
    const releaseDirectory = resolve('src-tauri/target/release')
    const manifest = createUpdateManifest(
      {
        version: process.env.RELEASE_VERSION ?? '',
        tag: process.env.RELEASE_TAG ?? '',
        repository: process.env.GITHUB_REPOSITORY ?? '',
        sourceCommit: process.env.RELEASE_SOURCE_COMMIT ?? '',
        publishedAt: process.env.RELEASE_PUBLISHED_AT ?? '',
        notes: process.env.RELEASE_NOTES ?? '',
      },
      (relativePath) => readFileSync(resolve(releaseDirectory, relativePath)),
    )
    writeFileSync(resolve(releaseDirectory, 'latest.json'), serialiseManifest(manifest))
    console.log(`Created update manifest for ${manifest.version} (installer and portable)`)
  } catch (error) {
    console.error(error instanceof Error ? error.message : 'Could not create the update manifest')
    process.exitCode = 1
  }
}
