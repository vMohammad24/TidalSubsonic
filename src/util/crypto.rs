use std::sync::LazyLock;

use aes_gcm::{
	Aes256Gcm, Key, Nonce,
	aead::{Aead, AeadCore, KeyInit, OsRng},
};
use base64::Engine;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
	#[error("Failed to decode base64: {0}")]
	Base64Decode(#[from] base64::DecodeError),
	#[error("Encryption failed: {0}")]
	EncryptionFailed(String),
	#[error("Decryption failed: {0}")]
	DecryptionFailed(String),
	#[error("Ciphertext too short to contain nonce")]
	InvalidLength,
}

static CIPHER: LazyLock<Aes256Gcm> = LazyLock::new(|| {
	let key_str = std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set");
	let key_bytes = base64::engine::general_purpose::STANDARD
		.decode(&key_str)
		.expect("Invalid Base64 in ENCRYPTION_KEY");

	Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes))
});

pub fn encrypt_string(plain_password: &str) -> Result<String, CryptoError> {
	let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

	let ciphertext = CIPHER
		.encrypt(&nonce, plain_password.as_bytes())
		.map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

	let mut combined = nonce.to_vec();
	combined.extend_from_slice(&ciphertext);

	Ok(base64::engine::general_purpose::STANDARD.encode(combined))
}

pub fn decrypt_string(encrypted_b64: &str) -> Result<String, CryptoError> {
	let combined = base64::engine::general_purpose::STANDARD.decode(encrypted_b64)?;

	if combined.len() < 12 {
		return Err(CryptoError::InvalidLength);
	}

	let (nonce_bytes, ciphertext) = combined.split_at(12);
	let nonce = Nonce::from_slice(nonce_bytes);

	let decrypted = CIPHER
		.decrypt(nonce, ciphertext)
		.map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

	String::from_utf8(decrypted).map_err(|_| CryptoError::DecryptionFailed("Invalid UTF-8".into()))
}
