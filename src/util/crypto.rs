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

	let mut combined = Vec::with_capacity(12 + ciphertext.len());
	combined.extend_from_slice(&nonce);
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_encrypt_decrypt_roundtrip() {
		let secret = "super_secret_password_123!";
		let encrypted = encrypt_string(secret).expect("Encryption failed");
		assert_ne!(secret, encrypted);

		let decrypted = decrypt_string(&encrypted).expect("Decryption failed");
		assert_eq!(secret, decrypted);
	}

	#[test]
	fn test_decrypt_invalid_base64() {
		let result = decrypt_string("invalid_base64!!!");
		assert!(matches!(result, Err(CryptoError::Base64Decode(_))));
	}

	#[test]
	fn test_decrypt_short_ciphertext() {
		let short_b64 = base64::engine::general_purpose::STANDARD.encode([1u8; 5]);
		let result = decrypt_string(&short_b64);
		assert!(matches!(result, Err(CryptoError::InvalidLength)));
	}

	#[test]
	fn test_decrypt_corrupted_ciphertext() {
		let corrupted_b64 = base64::engine::general_purpose::STANDARD.encode([0u8; 30]);
		let result = decrypt_string(&corrupted_b64);
		assert!(matches!(result, Err(CryptoError::DecryptionFailed(_))));
	}
}
