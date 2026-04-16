use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Encoding {
    #[default]
    Utf8,
    Hex,
    Base64,
}

impl Encoding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Utf8 => "utf8",
            Self::Hex => "hex",
            Self::Base64 => "base64",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error("field '{field}' is not valid UTF-8; retry with {field}_encoding=\"base64\"")]
    InvalidUtf8 { field: &'static str },
    #[error("field '{field}' is not valid hex: {source}")]
    InvalidHex {
        field: &'static str,
        #[source]
        source: hex::FromHexError,
    },
    #[error("field '{field}' is not valid base64: {source}")]
    InvalidBase64 {
        field: &'static str,
        #[source]
        source: base64::DecodeError,
    },
}

pub fn decode(field: &'static str, s: &str, enc: Encoding) -> Result<Vec<u8>, EncodingError> {
    match enc {
        Encoding::Utf8 => Ok(s.as_bytes().to_vec()),
        Encoding::Hex => {
            hex::decode(s).map_err(|source| EncodingError::InvalidHex { field, source })
        }
        Encoding::Base64 => B64
            .decode(s)
            .map_err(|source| EncodingError::InvalidBase64 { field, source }),
    }
}

pub fn encode(field: &'static str, bytes: &[u8], enc: Encoding) -> Result<String, EncodingError> {
    match enc {
        Encoding::Utf8 => std::str::from_utf8(bytes)
            .map(|s| s.to_owned())
            .map_err(|_| EncodingError::InvalidUtf8 { field }),
        Encoding::Hex => Ok(hex::encode(bytes)),
        Encoding::Base64 => Ok(B64.encode(bytes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_utf8() {
        let bytes = decode("key", "users/42", Encoding::Utf8).unwrap();
        assert_eq!(bytes, b"users/42");
        assert_eq!(encode("value", &bytes, Encoding::Utf8).unwrap(), "users/42");
    }

    #[test]
    fn roundtrip_hex() {
        let bytes = decode("key", "a1b2c3", Encoding::Hex).unwrap();
        assert_eq!(bytes, [0xa1, 0xb2, 0xc3]);
        assert_eq!(encode("value", &bytes, Encoding::Hex).unwrap(), "a1b2c3");
    }

    #[test]
    fn roundtrip_base64() {
        let bytes = decode("key", "obLD", Encoding::Base64).unwrap();
        assert_eq!(bytes, [0xa1, 0xb2, 0xc3]);
        assert_eq!(encode("value", &bytes, Encoding::Base64).unwrap(), "obLD");
    }

    #[test]
    fn utf8_encode_fails_on_non_utf8() {
        let err = encode("value", &[0xff, 0xfe], Encoding::Utf8).unwrap_err();
        assert!(matches!(err, EncodingError::InvalidUtf8 { field: "value" }));
    }

    #[test]
    fn hex_decode_fails_on_odd_length() {
        let err = decode("key", "abc", Encoding::Hex).unwrap_err();
        assert!(matches!(
            err,
            EncodingError::InvalidHex { field: "key", .. }
        ));
    }
}
