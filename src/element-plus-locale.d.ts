/**
 * Ambient type shim for Element Plus locale bundles.
 *
 * Element Plus ships `dist/locale/*.mjs` but its `package.json`
 * `exports` field only advertises the main entry point, so
 * `vue-tsc` can't infer these module paths on its own and emits
 * `TS7016: Could not find a declaration file for module …`.
 *
 * This is the workaround the Element Plus maintainers recommend
 * (see `element-plus/element-plus` issue tracker — one-line
 * ambient `declare module` per locale file path used). Once
 * Element Plus adds proper subpath exports, this file becomes dead
 * code and can be deleted.
 */

declare module 'element-plus/dist/locale/*.mjs' {
  import type { Language } from 'element-plus/es/locale'

  const locale: Language
  export default locale
}
