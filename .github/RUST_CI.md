# Rust CI maintenance

The Rust version is pinned in `workflows/ci.yml`. Both pull request checks and
scheduled cache warming use that workflow's Rust job, including the same runner,
toolchain, Cargo configuration, Clippy command and test command.

## Toolchain updates

`.github/renovate.json` allows only the Rust compiler version in `ci.yml` to be
updated. Renovate may create upgrade pull requests on the first day of each month
in `Asia/Taipei`. Existing branches can still be refreshed outside that window.
It requests review from `YCC3741`, adds the following trailer to its original
upgrade commits, and does not merge automatically:

```text
Co-authored-by: YCC3741 <mo0307b1006@gmail.com>
```

The free [Renovate GitHub App](https://github.com/apps/renovate) must be installed
and granted access to `pungin/Beanfun` by someone with the required repository
administration permissions. Merge the configuration into `code` before expecting
scheduled upgrade PRs. The repository configuration alone does not install the App.

Workflow and Cargo configuration changes trigger Rust checks. An upgrade must
pass formatting, Clippy with warnings denied, and the existing test suite before
the maintainer decides whether to merge it. These checks do not exercise live
account operations or build a release installer.

## Cache warming

The schedule runs at 03:17 UTC (11:17 Taipei) on days 1, 7, 13, 19, 25 and 31
where those dates exist. The intended gap is at most six days. Scheduled runs
execute on the default branch, `code`, and skip the frontend job.

Only successful runs on `code` save Rust caches. Pull requests restore these
caches without creating their own isolated copies. Rust-version changes require
a fresh dependency build; cache warming cannot eliminate the first build with a
new compiler. The application crate itself is still compiled on subsequent runs.
Warming produces no source commit or co-author attribution.

After merging, the push run seeds the shared cache. Use **Run workflow** on `code`
to refresh it manually; manual runs also execute frontend checks.

## Free-service limits

- Use [Mend Renovate Community Cloud](https://docs.mend.io/renovate/latest/mend-renovate-cloud-resource-tiers),
  which has a free tier for public repositories.
- Keep the repository public and use standard GitHub-hosted runners. Keep the
  repository's cache storage limit at the included 10 GB; do not enable paid cache
  expansion. This workflow uses the existing GitHub cache and requires no paid
  runner, external cache service or additional secret. See
  [GitHub Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions).
- GitHub can delay scheduled runs and disables public-repository schedules after
  60 days without repository activity. A cold build remains correct if a cache
  expires or is evicted. See
  [scheduled workflow behaviour](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#schedule).
