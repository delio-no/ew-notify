//! Windows DPAPI protection for sensitive data (e.g. Telegram bot token).

use anyhow::Result;

#[cfg(windows)]
mod win {
    use anyhow::{anyhow, Result};
    use std::slice;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HLOCAL;
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
    };
    use windows::Win32::System::Memory::LocalFree;

    pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let in_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();

        unsafe {
            let ok = CryptProtectData(
                &in_blob,
                PCWSTR::null(),
                None,
                None,
                None,
                0,
                &mut out_blob,
            );

            if !ok.as_bool() {
                return Err(anyhow!(
                    "CryptProtectData failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let slice = slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
            let result = slice.to_vec();
            let _ = LocalFree(HLOCAL(out_blob.pbData as isize));
            Ok(result)
        }
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let in_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();

        unsafe {
            let ok = CryptUnprotectData(
                &in_blob,
                None,
                None,
                None,
                None,
                0,
                &mut out_blob,
            );

            if !ok.as_bool() {
                return Err(anyhow!(
                    "CryptUnprotectData failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let slice = slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
            let result = slice.to_vec();
            let _ = LocalFree(HLOCAL(out_blob.pbData as isize));
            Ok(result)
        }
    }
}

#[cfg(not(windows))]
mod win {
    use anyhow::Result;

    pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
}

const ENC_PREFIX: &str = "enc:dpapi:";

/// Encrypt plaintext string into `enc:dpapi:<hex>` (or returns empty if empty).
pub fn encrypt_string(plain: &str) -> Result<String> {
    let trimmed = plain.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    // If already encrypted, don't re-encrypt
    if trimmed.starts_with(ENC_PREFIX) {
        return Ok(trimmed.to_string());
    }
    let encrypted_bytes = win::protect(trimmed.as_bytes())?;
    let hex: String = encrypted_bytes.iter().map(|b| format!("{:02X}", b)).collect();
    Ok(format!("{ENC_PREFIX}{hex}"))
}

/// Decrypt `enc:dpapi:<hex>` into plaintext UTF-8 string.
/// If string does not have `enc:dpapi:` prefix, it is assumed to be already plaintext.
pub fn decrypt_string(stored: &str) -> Result<String> {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if !trimmed.starts_with(ENC_PREFIX) {
        // Plaintext backwards-compatibility
        return Ok(trimmed.to_string());
    }
    let hex_part = &trimmed[ENC_PREFIX.len()..];
    let bytes = crate::packet::parse_hex(hex_part)?;
    let decrypted_bytes = win::unprotect(&bytes)?;
    let plain = String::from_utf8(decrypted_bytes)
        .map_err(|e| anyhow::anyhow!("Decrypted token is not valid UTF-8: {e}"))?;
    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let secret = "123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ_123456";
        let encrypted = encrypt_string(secret).expect("encryption failed");
        assert!(encrypted.starts_with(ENC_PREFIX));
        assert_ne!(encrypted, secret);

        let decrypted = decrypt_string(&encrypted).expect("decryption failed");
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn test_plaintext_fallback() {
        let plain = "123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ_123456";
        let res = decrypt_string(plain).expect("fallback failed");
        assert_eq!(res, plain);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(encrypt_string("").unwrap(), "");
        assert_eq!(decrypt_string("").unwrap(), "");
        assert_eq!(encrypt_string("   ").unwrap(), "");
    }

    #[test]
    fn test_idempotent_encrypt() {
        let secret = "my_token";
        let enc1 = encrypt_string(secret).unwrap();
        let enc2 = encrypt_string(&enc1).unwrap();
        assert_eq!(enc1, enc2);
    }
}
