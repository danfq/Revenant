//! primitives for ios backup decryption
//!
//! pbkdf2 key derivation, rfc3394 aes key unwrap and aes-256-cbc, built on
//! the raw aes block cipher so the behaviour matches apple's backupagent

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes256;
use sha1::Sha1;
use sha2::Sha256;

/// derive a key with pbkdf2-hmac-sha1
pub fn pbkdf2_sha1(password: &[u8], salt: &[u8], rounds: u32, out_len: usize) -> Vec<u8> {
    let mut out = vec![0u8; out_len];
    pbkdf2::pbkdf2_hmac::<Sha1>(password, salt, rounds, &mut out);
    out
}

/// derive a key with pbkdf2-hmac-sha256
pub fn pbkdf2_sha256(password: &[u8], salt: &[u8], rounds: u32, out_len: usize) -> Vec<u8> {
    let mut out = vec![0u8; out_len];
    pbkdf2::pbkdf2_hmac::<Sha256>(password, salt, rounds, &mut out);
    out
}

/// decrypt a single 16 byte block with aes-256 ecb
fn decrypt_block(cipher: &Aes256, block: &[u8]) -> [u8; 16] {
    let mut b = GenericArray::clone_from_slice(block);
    cipher.decrypt_block(&mut b);
    let mut out = [0u8; 16];
    out.copy_from_slice(&b);
    out
}

/// rfc3394 aes key unwrap
///
/// returns the plaintext key, or none if the integrity check fails (wrong kek)
pub fn aes256_unwrap(kek: &[u8], wrapped: &[u8]) -> Option<Vec<u8>> {
    if kek.len() != 32 || wrapped.is_empty() || wrapped.len() % 8 != 0 {
        return None;
    }
    let cipher = Aes256::new(GenericArray::from_slice(kek));
    let n = wrapped.len() / 8 - 1;
    let mut a = u64::from_be_bytes(wrapped[0..8].try_into().ok()?);
    let mut r: Vec<u64> = (1..=n)
        .map(|i| u64::from_be_bytes(wrapped[i * 8..i * 8 + 8].try_into().unwrap()))
        .collect();

    for j in (0..6).rev() {
        for i in (1..=n).rev() {
            let t = (n as u64) * j + i as u64;
            let mut buf = [0u8; 16];
            buf[0..8].copy_from_slice(&(a ^ t).to_be_bytes());
            buf[8..16].copy_from_slice(&r[i - 1].to_be_bytes());
            let dec = decrypt_block(&cipher, &buf);
            a = u64::from_be_bytes(dec[0..8].try_into().unwrap());
            r[i - 1] = u64::from_be_bytes(dec[8..16].try_into().unwrap());
        }
    }
    if a != 0xa6a6_a6a6_a6a6_a6a6 {
        return None;
    }
    let mut out = Vec::with_capacity(n * 8);
    for word in r {
        out.extend_from_slice(&word.to_be_bytes());
    }
    Some(out)
}

/// decrypt aes-256-cbc with no padding removal
///
/// ios encrypts backup payloads with a zero iv and stores the real plaintext
/// length separately, so the caller truncates the result
pub fn aes256_cbc_decrypt(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut out = Vec::with_capacity(data.len());
    let mut prev = *iv;
    for chunk in data.chunks_exact(16) {
        let dec = decrypt_block(&cipher, chunk);
        let mut plain = [0u8; 16];
        for i in 0..16 {
            plain[i] = dec[i] ^ prev[i];
        }
        out.extend_from_slice(&plain);
        prev.copy_from_slice(chunk);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn rfc3394_aes256_unwrap_256bit_key() {
        // rfc 3394 section 4.6 test vector
        let kek = hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        let wrapped = hex(
            "28c9f404c4b810f4cbccb35cfb87f8263f5786e2d80ed326cbc7f0e71a99f43bfb988b9b7a02dd21",
        );
        let expected = hex("00112233445566778899aabbccddeeff000102030405060708090a0b0c0d0e0f");
        assert_eq!(aes256_unwrap(&kek, &wrapped), Some(expected));
    }

    #[test]
    fn rfc3394_unwrap_rejects_wrong_kek() {
        let kek = [0u8; 32];
        let wrapped = hex(
            "28c9f404c4b810f4cbccb35cfb87f8263f5786e2d80ed326cbc7f0e71a99f43bfb988b9b7a02dd21",
        );
        assert_eq!(aes256_unwrap(&kek, &wrapped), None);
    }

    #[test]
    fn rfc6070_pbkdf2_sha1() {
        // rfc 6070 vector: p="password" s="salt" c=1 dklen=20
        let out = pbkdf2_sha1(b"password", b"salt", 1, 20);
        assert_eq!(out, hex("0c60c80f961f0e71f3a9b524af6012062fe037a6"));
    }

    #[test]
    fn pbkdf2_sha256_known_vector() {
        // p="password" s="salt" c=1 dklen=32
        let out = pbkdf2_sha256(b"password", b"salt", 1, 32);
        assert_eq!(
            out,
            hex("120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b")
        );
    }

    #[test]
    fn cbc_decrypt_inverts_cbc_encrypt() {
        // self-contained: cbc-encrypt with the block cipher, then assert our
        // cbc decrypt reproduces the plaintext across several blocks
        use aes::cipher::{BlockEncrypt, KeyInit};
        let key = hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let iv = [0u8; 16];
        let pt: Vec<u8> = (0..48u8).collect();
        let cipher = Aes256::new(GenericArray::from_slice(&key));
        let mut ct = Vec::new();
        let mut prev = iv;
        for chunk in pt.chunks_exact(16) {
            let mut b = [0u8; 16];
            for i in 0..16 {
                b[i] = chunk[i] ^ prev[i];
            }
            let mut ga = GenericArray::clone_from_slice(&b);
            cipher.encrypt_block(&mut ga);
            prev.copy_from_slice(&ga);
            ct.extend_from_slice(&ga);
        }
        assert_eq!(aes256_cbc_decrypt(&key, &iv, &ct), pt);
    }
}
