use std::fmt;
use std::string::FromUtf8Error;

use crate::lexer::impl_lexer::Lexer;
use crate::lexer::impl_lexer::ParserLanguage;

#[derive(Debug, thiserror::Error)]
pub enum StrLitDecodeError {
    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),
    #[error("String literal decode error")]
    OtherError,
}

pub type StrLitDecodeResult<T> = Result<T, StrLitDecodeError>;

/// String literal, both `string` and `bytes`.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct StrLit {
    pub escaped: String,
}

impl fmt::Display for StrLit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\"", &self.escaped)
    }
}

impl StrLit {
    /// May fail if not valid UTF8
    pub fn decode_utf8(&self) -> StrLitDecodeResult<String> {
        let mut lexer = Lexer::new(&self.escaped, ParserLanguage::Json);
        let mut r = Vec::new();
        while !lexer.eof() {
            r.push(
                lexer
                    .next_byte_value()
                    .map_err(|_| StrLitDecodeError::OtherError)?,
            );
        }
        Ok(String::from_utf8(r)?)
    }

    pub fn decode_bytes(&self) -> StrLitDecodeResult<Vec<u8>> {
        let mut lexer = Lexer::new(&self.escaped, ParserLanguage::Json);
        let mut r = Vec::new();
        while !lexer.eof() {
            r.push(
                lexer
                    .next_byte_value()
                    .map_err(|_| StrLitDecodeError::OtherError)?,
            );
        }
        Ok(r)
    }

    pub fn quoted(&self) -> String {
        format!("\"{}\"", self.escaped)
    }
}
