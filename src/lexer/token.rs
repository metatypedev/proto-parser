use crate::lexer::impl_lexer::LexerError;
use crate::lexer::impl_lexer::LexerResult;
use crate::lexer::jsonnumberlit::JsonNumberLit;
use crate::lexer::loc::Loc;
use crate::lexer::numlit::NumLit;
use crate::lexer::strlit::StrLit;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Ident(String),
    Symbol(char),
    IntLit(u64),
    FloatLit(f64),
    JsonNumber(JsonNumberLit),
    StrLit(StrLit),
}

use Token::*;

impl Token {
    /// Back to original
    pub fn format(&self) -> String {
        match self {
            Ident(ref s) => s.clone(),
            Symbol(c) => c.to_string(),
            IntLit(ref i) => i.to_string(),
            StrLit(ref s) => s.quoted(),
            FloatLit(ref f) => f.to_string(),
            JsonNumber(ref f) => f.to_string(),
        }
    }

    pub fn to_num_lit(&self) -> LexerResult<NumLit> {
        match self {
            IntLit(i) => Ok(NumLit::U64(*i)),
            FloatLit(f) => Ok(NumLit::F64(*f)),
            _ => Err(LexerError::IncorrectInput),
        }
    }
}

#[derive(Clone)]
pub struct TokenWithLocation {
    pub token: Token,
    pub loc: Loc,
}
