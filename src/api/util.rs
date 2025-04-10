use aes::cipher::KeyIvInit;
use rand::RngCore;
use rand::rng;  
use aes::cipher::{BlockDecryptMut, BlockEncryptMut};
use cbc::cipher::block_padding::Pkcs7;

pub type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
pub type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

pub fn encrypt(key: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    let mut iv = [0u8; 16];
    rng().fill_bytes(&mut iv);

    let cipher = Aes256CbcEnc::new_from_slices(key, &iv).ok()?;
    let e_bytes = cipher.encrypt_padded_vec_mut::<Pkcs7>(data);

    let mut result = iv.to_vec();
    result.extend(e_bytes);
    Some(result)
}

/// Decrypts AES-256-CBC data, extracting the IV from the first 16 bytes
pub fn decrypt(key: &[u8], encrypted_data: &[u8]) -> Option<Vec<u8>> {
    if encrypted_data.len() < 16 {
        return None;
    }
    let (iv, ciphertext) = encrypted_data.split_at(16);
    let cipher = Aes256CbcDec::new_from_slices(key, iv).ok()?;
    let mut buffer = ciphertext.to_vec();
    cipher.decrypt_padded_vec_mut::<Pkcs7>(&mut buffer).ok()
}