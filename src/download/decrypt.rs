use aes::cipher::{BlockDecryptMut, KeyIvInit, StreamCipher};
use anyhow::{anyhow, Result};
use base64::Engine;

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

const MASTER_KEY_B64: &str = "UIlTTEMmmLfGowo/UC60x2H45W6MdGgTRfo/umg4754=";

/// Decrypt the Tidal security token to extract the AES key and nonce.
pub fn decrypt_security_token(security_token: &str) -> Result<([u8; 16], [u8; 8])> {
    let b64 = base64::engine::general_purpose::STANDARD;

    let master_key = b64
        .decode(MASTER_KEY_B64)
        .map_err(|e| anyhow!("Failed to decode master key: {}", e))?;

    let token_bytes = b64
        .decode(security_token)
        .map_err(|e| anyhow!("Failed to decode security token: {}", e))?;

    if token_bytes.len() < 32 {
        return Err(anyhow!("Security token too short"));
    }

    let iv = &token_bytes[..16];
    let ciphertext = &token_bytes[16..];

    // Pad to multiple of 16
    let mut buf = ciphertext.to_vec();
    let pad = (16 - buf.len() % 16) % 16;
    buf.extend(std::iter::repeat_n(0u8, pad));
    let decryptor = Aes256CbcDec::new_from_slices(&master_key, iv)
        .map_err(|e| anyhow!("AES init failed: {}", e))?;

    let decrypted = decryptor
        .decrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buf)
        .map_err(|e| anyhow!("AES-CBC decryption failed: {:?}", e))?;

    if decrypted.len() < 24 {
        return Err(anyhow!("Decrypted token too short"));
    }

    let mut key = [0u8; 16];
    key.copy_from_slice(&decrypted[0..16]);
    let mut nonce = [0u8; 8];
    nonce.copy_from_slice(&decrypted[16..24]);

    Ok((key, nonce))
}

/// Decrypt an encrypted audio file using AES-128-CTR.
pub fn decrypt_file(data: &[u8], key: &[u8; 16], nonce: &[u8; 8]) -> Vec<u8> {
    let mut iv = [0u8; 16];
    iv[..8].copy_from_slice(nonce);

    type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;
    let mut cipher = Aes128Ctr::new_from_slices(key, &iv).expect("Valid key and IV");
    let mut buf = data.to_vec();
    cipher.apply_keystream(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decrypt_file_roundtrip() {
        type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;

        let key = [0x42u8; 16];
        let nonce = [0x13u8; 8];
        let plaintext = b"Hello, Tidal World! AES-128-CTR roundtrip test.";

        let mut iv = [0u8; 16];
        iv[..8].copy_from_slice(&nonce);
        let mut cipher = Aes128Ctr::new_from_slices(&key, &iv).unwrap();
        let mut ciphertext = plaintext.to_vec();
        cipher.apply_keystream(&mut ciphertext);

        let decrypted = decrypt_file(&ciphertext, &key, &nonce);
        assert_eq!(decrypted, plaintext);
    }
}
