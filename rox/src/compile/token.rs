#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token<'src> {
    pub kind: Kind,
    pub lexeme: &'src str,
    pub line: u32,
}

impl Token<'static> {
    pub const fn synthetic(lexeme: &'static str, line: u32) -> Self {
        Self {
            kind: Kind::Identifier,
            lexeme,
            line,
        }
    }
}

impl Default for Token<'static> {
    fn default() -> Self {
        Self {
            kind: Kind::False,
            lexeme: "",
            line: u32::MAX,
        }
    }
}

#[derive(Debug, Copy, PartialEq, Clone)]
#[repr(u8)]
pub enum Kind {
    // single char tokens
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    SemiColon,
    Slash,
    Star,
    // 1 or 2 character tokens
    Bang,
    BangEquals,
    Equal,
    EqualEquals,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    // Literals
    Identifier,
    String,
    Number,
    // Keywords
    And,
    Class,
    Else,
    False,
    For,
    Fun,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Var,
    While,
    Const,

    Error,
    EOF,
}
