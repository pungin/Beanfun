/**
 * Commitlint configuration (CI-only)
 *
 * Runs in .github/workflows/commitlint.yml on pull requests targeting the
 * `code` branch. Extends conventional-commits with these repo-specific tweaks:
 *
 * - `header-max-length` raised to 120 (default 72 is too strict for scoped
 *   subjects like `feat(next): scaffold Tauri v2 + Vue 3 TS project (P0 chunk 1)`).
 * - `body-max-line-length` / `footer-max-line-length` disabled to avoid
 *   rejecting Chinese commit bodies (CJK characters count as multi-byte).
 * - `scope-enum` disabled: this monorepo mixes scopes such as `next`,
 *   `updater`, `ui`, `deps`, `ci`, etc., so a closed enum is counter-
 *   productive.
 * - Dependabot ("Bump X from A to B") and merge commits are ignored.
 */
export default {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'header-max-length': [2, 'always', 120],
    'body-max-line-length': [0],
    'footer-max-line-length': [0],
    'scope-enum': [0],
  },
  ignores: [
    (message) => /^Bump\s+.+\s+from\s+.+\s+to\s+.+/i.test(message),
    (message) => /^Merge (pull request|branch)/i.test(message),
  ],
}
