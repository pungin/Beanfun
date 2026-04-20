//! DPAPI `CryptProtectData` / `CryptUnprotectData` wrappers, `CurrentUser`
//! scope.
//!
//! Ports the DPAPI calls from WPF `AccountManager.ciphertext` and
//! `AccountManager.readRawData` (`Beanfun/Helper/AccountManager.cs`
//! L207-267).
//!
//! # Scope
//!
//! All operations run under `CurrentUser` scope (the default when
//! `CRYPTPROTECT_LOCAL_MACHINE` is not set in `dwFlags`). Ciphertext
//! produced on one machine by one user account **cannot** be
//! unprotected by another account or another machine — this is an
//! intentional property inherited from WPF and is what makes DPAPI a
//! meaningful protection layer for a user-local credential cache.
//!
//! # Entropy
//!
//! Callers pass an `entropy` salt (typically the 8-char UTF-8 bytes of
//! [`super::Entropy`] stored in `HKCU\SOFTWARE\BEANFUN\ENTROPY`). The
//! same bytes must be supplied at both protect and unprotect time; a
//! mismatch surfaces as a `StorageError::Dpapi` error.
//!
//! # Memory management
//!
//! Both APIs write their output into a `CRYPT_INTEGER_BLOB` whose
//! `pbData` is allocated with `LocalAlloc`. This module always copies
//! the bytes into a `Vec<u8>` before calling `LocalFree`, so the
//! returned `Vec` is owned by the Rust allocator and callers never see
//! a Win32 handle.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

use super::error::StorageError;

/// Protect `plain` under DPAPI `CurrentUser` scope using `entropy` as
/// the optional secondary secret.
///
/// Returns a freshly-allocated `Vec<u8>` of ciphertext suitable for
/// persisting to disk or sharing between processes running as the same
/// user account.
pub fn dpapi_protect(plain: &[u8], entropy: &[u8]) -> Result<Vec<u8>, StorageError> {
    // The Win32 API takes `*const CRYPT_INTEGER_BLOB` with `pbData: *mut u8`
    // in the struct, but it does not mutate the input buffer for Protect —
    // the `*mut` is purely a convention. Casting away the immutability via
    // `as *mut u8` is safe because the callee treats it as read-only.
    let data_in = CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        pbData: plain.as_ptr() as *mut u8,
    };
    let data_entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy.len() as u32,
        pbData: entropy.as_ptr() as *mut u8,
    };
    let mut data_out = CRYPT_INTEGER_BLOB::default();

    // Safety: `data_in` / `data_entropy` point to valid buffers alive for
    // the duration of the call (`plain` and `entropy` outlive this block
    // because they are borrowed by reference). `data_out` is written by
    // the API; on success we copy the result and free the Win32 buffer
    // below.
    unsafe {
        CryptProtectData(
            &data_in,
            PCWSTR::null(),
            Some(&data_entropy),
            None,
            None,
            0,
            &mut data_out,
        )
        .map_err(|e| StorageError::Dpapi {
            operation: "CryptProtectData",
            message: e.to_string(),
        })?;
    }

    let cipher =
        unsafe { std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec() };

    // Safety: `data_out.pbData` was allocated by the Win32 API via
    // `LocalAlloc`. We're releasing it immediately after copying out.
    unsafe {
        let _ = LocalFree(HLOCAL(data_out.pbData as _));
    }

    Ok(cipher)
}

/// Unprotect `cipher` under DPAPI `CurrentUser` scope, supplying
/// `entropy` as the same salt that was passed to
/// [`dpapi_protect`].
///
/// Fails with `StorageError::Dpapi` on wrong entropy, tampered
/// ciphertext, or a ciphertext produced by a different user / machine.
pub fn dpapi_unprotect(cipher: &[u8], entropy: &[u8]) -> Result<Vec<u8>, StorageError> {
    let data_in = CRYPT_INTEGER_BLOB {
        cbData: cipher.len() as u32,
        pbData: cipher.as_ptr() as *mut u8,
    };
    let data_entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy.len() as u32,
        pbData: entropy.as_ptr() as *mut u8,
    };
    let mut data_out = CRYPT_INTEGER_BLOB::default();

    // Safety: see `dpapi_protect`.
    unsafe {
        CryptUnprotectData(
            &data_in,
            None,
            Some(&data_entropy),
            None,
            None,
            0,
            &mut data_out,
        )
        .map_err(|e| StorageError::Dpapi {
            operation: "CryptUnprotectData",
            message: e.to_string(),
        })?;
    }

    let plain =
        unsafe { std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec() };

    unsafe {
        let _ = LocalFree(HLOCAL(data_out.pbData as _));
    }

    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_hello_world() {
        let entropy = b"AB12CD34";
        let cipher = dpapi_protect(b"hello, world", entropy).expect("protect");
        assert_ne!(cipher.as_slice(), b"hello, world");
        let plain = dpapi_unprotect(&cipher, entropy).expect("unprotect");
        assert_eq!(plain, b"hello, world");
    }

    #[test]
    fn wrong_entropy_fails_to_unprotect() {
        let cipher = dpapi_protect(b"secret payload", b"AB12CD34").expect("protect");
        let err = dpapi_unprotect(&cipher, b"XY78EF90")
            .expect_err("unprotect with wrong entropy must fail");
        assert!(
            matches!(
                err,
                StorageError::Dpapi {
                    operation: "CryptUnprotectData",
                    ..
                }
            ),
            "expected DPAPI unprotect error, got {err:?}"
        );
    }

    #[test]
    fn round_trip_empty_payload() {
        let entropy = b"AB12CD34";
        let cipher = dpapi_protect(b"", entropy).expect("protect empty");
        let plain = dpapi_unprotect(&cipher, entropy).expect("unprotect empty");
        assert_eq!(plain, b"");
    }

    #[test]
    fn round_trip_large_payload() {
        let entropy = b"AB12CD34";
        let large: Vec<u8> = (0..4096).map(|i| (i % 251) as u8).collect();
        let cipher = dpapi_protect(&large, entropy).expect("protect 4KB");
        let plain = dpapi_unprotect(&cipher, entropy).expect("unprotect 4KB");
        assert_eq!(plain, large);
    }

    #[test]
    fn round_trip_empty_entropy() {
        // WPF passes an 8-char entropy in practice, but DPAPI does not
        // require one — an empty entropy (different from a mismatched
        // one) should still round-trip.
        let cipher = dpapi_protect(b"data", b"").expect("protect no-entropy");
        let plain = dpapi_unprotect(&cipher, b"").expect("unprotect no-entropy");
        assert_eq!(plain, b"data");
    }
}
