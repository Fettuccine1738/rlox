#[derive(Debug)]
pub struct Scanner<'a> {
    source: &'a str,
    start: usize,
    current: usize,
    line: u32,
}

pub struct Token<'a> {
    pub kind: Kind,
    pub lexeme: &'a str,
    // start: usize, // ptr to the first character of our lexeme
    // length: usize, // number of character it should contain.
    pub line: u32
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Kind {
    // single char tokens 
    LeftParen, RightParen, LeftBrace, RightBrace,
    Comma, Dot, Minus, Plus, SemiColon, Slash, Star,
    // 1 or 2 character tokens
    Bang, BangEquals, Equal, EqualEquals, Greater, GreaterEqual,
    Less, LessEqual,
    // Literals
    Identifier, String, Number,
    // Keywords
    And, Class, Else, False, For, Fun, If, Nil, Or, Print, Return, Super, 
    This, True, Var, While,

    Error, EOF
}

impl<'a> Scanner<'a> {
    pub fn new(source_: &'a str) -> Self {
        Self {
            source: source_,
            start: 0,
            current: 0,
            line: 1,
        }
    }

    pub fn scan_token(&mut self) -> Token {
        self.start = self.current;

        if self.is_at_end() {
            self.make_token(Kind::EOF);
        } 

        match self.advance() {
            '(' => self.make_token(Kind::LeftParen),
            ')' => self.make_token(Kind::RightParen),
            '{' => self.make_token(Kind::LeftBrace),
            '}' => self.make_token(Kind::RightBrace),
            ';' => self.make_token(Kind::SemiColon),
            ',' => self.make_token(Kind::Comma),
            '.' => self.make_token(Kind::Dot),
            '-' => self.make_token(Kind::Minus),
            '+' => self.make_token(Kind::Plus),
            '*' => self.make_token(Kind::Star),
            '/' => self.make_token(Kind::Slash),
            '!' => if self.match_char('=') { self.make_token(Kind::BangEquals) } 
                    else { self.make_token(Kind::Equal) },
            '=' => if self.match_char('=') { self.make_token(Kind::EqualEquals) } 
                    else { self.make_token(Kind::Equal) },
            '<' => if self.match_char('=') { self.make_token(Kind::LessEqual) } 
                    else { self.make_token(Kind::Less) },
            '>' => if self.match_char('=') { self.make_token(Kind::GreaterEqual) } 
                    else { self.make_token(Kind::Greater) },
            '"' => self.string(),
            _=> todo!()
        }; 
        self.error_token("Unexpected character.")
    }

    fn match_char(&mut self, expect: char) -> bool {
        if self.is_at_end() {
            false
        } else if expect == (self.source.as_bytes()[self.current] as char) {
            false
        } else {
            self.current += 1;
            true
        }
    } 

    fn skip_whitespace(&mut self) {
        // while let Some(' ' | '\r' | '\t') = self.peek() {
        //     let _ = self.advance();
        // }
        loop {
            let ch: Option<char> = self.peek();
            match ch {
                Some(' ' | '\r' | '\t') => {
                    let _ = self.advance(); 
                },
                Some('\n') => {
                    let _ = self.advance();
                    self.line += 1;
                }
                Some('/') => {
                    if let Some('/') = self.source[self.current+1..].chars().next() {
                        while self.peek().unwrap() != '\n'  && !self.is_at_end()  {
                            let _ = self.advance();
                        }
                    } else {
                        return;
                    }
                }
                _ => break,
            }
        }
    }

    fn string(&mut self) -> Token {
        while self.peek().unwrap() != '"'  && !self.is_at_end()  {
            if self.peek().unwrap() == '\n' {
                self.line += 1;
            }
            let _ = self.advance();
        }
        if self.is_at_end() {
            return self.error_token("Unterminated string found.");
        }
        // consume terminating '"'
        self.advance();
        self.make_token(Kind::String)
    }

    fn peek(&self) -> Option<char> {
        self.source[self.current..].chars().next()
    }

    // NOTE: On why using the Iterator to get the next char is efficient 
    // Rust does not allocate: Tiny Struct Chars { ptr ,end } (2 pointers on the stack)
    // Cost per call: no heap allcoation, no copying, just a few instructions, 
    fn advance(&mut self) -> char {
        // Cleaner and faster is using byte scanning, Lox only uses ascii 
        // let current = self.source[self.current..].chars().next();
        let b = self.source.as_bytes()[self.current];
        self.current += 1;
        b as char
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn make_token(&self, kind: Kind) -> Token<'a> {
        Token {
            kind: kind,
            lexeme: &self.source[self.start..self.current],
            line: self.line
        }
    }

    // lifetime of Token must be enclosed by msg's i.e
    // msg cannot be 'dropped' before Token.
    // ? Good memory model a combination of C code ::
    // ? char* start and int length is a perfect candidate for a
    // ? &str  
    fn error_token(&self, msg: &'a str) -> Token<'a> {
        Token {
            kind: Kind::Error,
            lexeme: msg,
            line: self.line
        }
    }
}
