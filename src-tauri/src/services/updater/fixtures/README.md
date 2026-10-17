# Sigstore verification fixtures

The attestation and small conda archive are unchanged public fixtures from the
official `sigstore-verify` crate, version 0.15.0 (Apache-2.0):

- Repository commit: `f950b1b683560aadf7737539f8d7eaa656a9f0c0`
- Source: <https://github.com/sigstore/sigstore-rust/tree/f950b1b683560aadf7737539f8d7eaa656a9f0c0/crates/sigstore-verify/test_data/bundles>
- Upstream licence: <https://github.com/sigstore/sigstore-rust/blob/f950b1b683560aadf7737539f8d7eaa656a9f0c0/LICENSE>
- Unmodified local licence: [LICENSE](LICENSE).
- Original signing workflow: `prefix-dev/sigstore-example/.github/workflows/action.yaml@refs/heads/main`

Tests verify the real certificate, SCT, transparency proof, DSSE signature and
artifact digest without network access. Separate tests reject this otherwise
valid signature under Beanfun's fixed release identity policy. These fixtures
are test data only; the archive is never installed or executed.

The upstream licence is reproduced unchanged in [LICENSE](LICENSE).
