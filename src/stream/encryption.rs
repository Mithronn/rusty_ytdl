use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, KeyIvInit};

use m3u8_rs::Key;
use reqwest::Url;

use crate::utils::make_absolute_url;
use crate::VideoError;

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

/// HLS encryption methods
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Encryption {
    None,
    Aes128 { key_uri: Url, iv: [u8; 16] },
    SampleAes,
}

impl Encryption {
    /// Check m3u8_key and return encryption.
    ///
    /// If encrypted, will make a query to the designated url to fetch the key
    pub async fn new(m3u8_key: &Key, base_url: &str, seq: u64) -> Result<Self, VideoError> {
        let encryption = match &m3u8_key {
            k if k.method.to_string() == *"NONE" => Self::None,
            k if k.method.to_string() == *"AES-128" => {
                if let Some(uri) = &k.uri {
                    // Bail if keyformat exists but is not "identity"
                    if let Some(keyformat) = &k.keyformat {
                        if keyformat != "identity" {
                            return Err(VideoError::EncryptionError(format!(
                                "Invalid keyformat: {}",
                                keyformat
                            )));
                        }
                    }

                    // Fetch key
                    let uri = make_absolute_url(base_url, uri)?;

                    // Parse IV
                    let mut iv = [0_u8; 16];
                    if let Some(iv_str) = &k.iv {
                        // IV is given separately
                        let iv_str = iv_str.trim_start_matches("0x");
                        hex::decode_to_slice(iv_str, &mut iv as &mut [u8])
                            .map_err(VideoError::HexError)?;
                    } else {
                        // Compute IV from segment sequence
                        iv[(16 - std::mem::size_of_val(&seq))..]
                            .copy_from_slice(&seq.to_be_bytes());
                    }

                    Self::Aes128 { key_uri: uri, iv }
                } else {
                    // Bail if no uri is found
                    return Err(VideoError::EncryptionError(
                        "No URI found for AES-128 key".to_string(),
                    ));
                }
            }
            k if k.method.to_string() == *"SAMPLE-AES" => {
                return Err(VideoError::EncryptionError(format!(
                    "Unimplemented encryption method: {}",
                    k.method
                )))
            }
            k => {
                return Err(VideoError::EncryptionError(format!(
                    "Invalid encryption method: {}",
                    k.method
                )))
            }
        };

        Ok(encryption)
    }

    /// Decrypt the given data
    pub async fn decrypt(
        &self,
        client: &reqwest_middleware::ClientWithMiddleware,
        data: &[u8],
    ) -> Result<Vec<u8>, VideoError> {
        let r = match self {
            Self::None => Vec::from(data),
            Self::Aes128 { key_uri, iv } => {
                let body = client.get(key_uri.clone()).send().await?.bytes().await?;
                let mut key = [0_u8; 16];
                key.copy_from_slice(&body[..16]);
                Aes128CbcDec::new(&key.into(), iv.into())
                    .decrypt_padded_vec_mut::<Pkcs7>(data)
                    .map_err(|e| VideoError::DecryptionError(e.to_string()))?
            }
            Self::SampleAes => unimplemented!(),
        };

        Ok(r)
    }
}
