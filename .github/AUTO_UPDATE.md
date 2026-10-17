# Application updates

Beanfun supports updates for both distributed Windows editions. The installed edition
downloads the NSIS installer; the portable edition downloads `Beanfun.exe` and replaces
the executable in its existing location. Updating must not delete saved accounts or
application settings. The application displays download progress and verifies the
release's keyless attestation and the executable's digest before installation.

The existing Stable and Beta channel selection still chooses the release. The updater
then reads `latest.json` from that exact release tag, rather than GitHub's global
`releases/latest` redirect. Draft releases are excluded from update checks. Both editions
use the same channel rules and release version.

With "Notify when a new version is available" enabled, the main window checks once
in the background after configuration and language initialisation. It only prompts
for a newer release, waits for any startup announcement to close, and never starts
a download without confirmation. No-update and network-error results stay silent
during startup. Manual checks remain available from the About page and report their
results. Both entry points share one update flow, so they cannot download or prompt
twice concurrently. An approved update opens the About page before downloading,
keeping progress and restart feedback visible in the selected language.

## Signing identity and maintainer access

The release workflow uses GitHub's official
[`actions/attest`](https://github.com/actions/attest) action, pinned to a reviewed commit.
GitHub Actions obtains an OIDC identity, signs with a temporary key and records the
attestation in Sigstore's public transparency log and GitHub's attestation service.
There is no long-lived update signing key to generate, back up, distribute or recover,
and no `TAURI_SIGNING_PRIVATE_KEY` secret is required.

The client trusts the release workflow identity, rather than an individual maintainer:

- Repository `pungin/Beanfun`, repository ID `155946625` and owner ID `11711288`.
- `.github/workflows/build-and-release.yml` on `refs/heads/code`.
- A manual `workflow_dispatch` run on a GitHub-hosted runner.
- The source commit recorded in the signing certificate, provenance and manifest.

Co-maintainers with write access can normally run this manual workflow without holding
a private-key copy or being the repository owner. Repository policies must permit the
pinned action and the job's `contents: write`, `id-token: write` and
`attestations: write` permissions. An administrator must resolve any applicable Actions
policy, environment restriction or branch protection that prevents publication or the
version bump. Protect access to the repository and review changes to the release
workflow: keyless signing authenticates the authorised build, not its code quality.

These identity checks deliberately exclude forks, renamed or transferred repositories,
other branches and other workflows. Changing the trusted identity requires a planned
client migration; it must not be bypassed to make an unexpected release installable.

## Release procedure

Continue to run **Build and Release** manually from `code` and choose `release` or `prerelease`.
No scheduled publication or automatic merge is introduced.

The job checks the repository identity, visibility, branch and trigger before building.
It builds both editions and creates the manifest from their exact bytes. The generator
rejects missing or malformed Windows executables, files above the client's 512 MiB
limit, mismatched release tags, invalid source commits or dates, and manifests larger
than 1 MiB. The action then creates one SLSA provenance attestation with three subjects:
the manifest and both executables. The workflow copies its JSON bundle unchanged into
the release assets; bundles above the client's 8 MiB limit are rejected.

Each release contains exactly these updater assets, alongside its normal release notes:

- `Beanfun.exe`.
- `Beanfun_<version>_x64-setup.exe`.
- `latest.json`, containing `schema_version: 1`, the plain `X.Y.Z` version, exact tag,
  `source_commit`, release notes, UTC publication date, and the URL, SHA-256 digest
  and byte length for each of `windows-x86_64` and `windows-x86_64-portable`.
- `latest.json.sigstore.json`, containing the Sigstore bundle for all three subjects.

All four assets are uploaded while the release is a draft. Only a successful upload
publishes the release, so clients cannot select a partially uploaded update. A failed
upload leaves a draft for the maintainer to inspect. Do not publish that draft manually
unless all four assets are present and valid. The release tag and manifest's
`source_commit` refer to the workflow's original `github.sha`, which matches the OIDC
source identity. The existing version bump commit remains after publication.

The application verifies the manifest's attestation before trusting its contents. It
checks the Sigstore certificate and transparency evidence, the expected workflow
identity, provenance and manifest digest, then the selected executable's digest and
length. A release URL, a SHA-256 string or unsigned JSON alone is not a trust anchor.
Never remove attestation verification to work around a failed update.

The bundle allows local cryptographic verification without a GitHub account or a
per-download GitHub attestation API request. The main application refreshes and caches
Sigstore trust metadata through TUF; the portable helper verifies that cache offline
from its embedded bootstrap root. This is not a promise that update checks never use
the network: release discovery, downloads and secure trust-metadata refresh still do.
Expired or invalid trust metadata must fail verification rather than bypass it.

## Initial rollout and verification

Users of older Beanfun builds must download and install or replace the executable once
to obtain the updater. Subsequent compatible releases can update from inside the app.
Both published editions must include the same trusted release identity and verification
policy.

The first production keyless release and the following two-version Windows acceptance
check are still pending. Before rollout, test two attested versions in a Windows test
account:

- Update the installed edition and confirm that it remains installed, starts again,
  and retains its accounts and settings.
- Update the portable edition in a writable folder, including a path with spaces or
  non-ASCII characters, and confirm it restarts from the original location.
- Check Stable and Beta selection, a cancelled confirmation, network failure, a
  missing manifest, invalid attestation and altered executable. A failed verification must
  leave the current executable usable.
- Check an unwritable portable location and a second process holding the executable.
  Confirm that a failed replacement preserves the original application and reports a
  useful error instead of claiming success.

Automated unit tests validate manifest generation and updater behaviour, but they do not
replace this attested installer and executable replacement check on Windows.

## Cost

Updates use this public repository's existing GitHub Releases and standard Windows
GitHub Actions runner. No external update server, paid service, larger runner or paid
code-signing certificate is required by this design. GitHub documents standard hosted
runners as free for public repositories in its
[Actions billing guidance](https://docs.github.com/en/billing/concepts/product-billing/github-actions).
GitHub's
[artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)
are available for public repositories on GitHub Free, and
[Sigstore's public-good services](https://docs.sigstore.dev/) are free. Downloads do not
create a per-install signing charge. Service availability and API rate limits still
apply; this change does not alter billing settings or create a paid service.

The attestation verifies the downloaded application's build identity; it does not
remove Windows SmartScreen prompts. A Windows Authenticode certificate is a separate,
optional distribution decision and is not configured by this workflow.
