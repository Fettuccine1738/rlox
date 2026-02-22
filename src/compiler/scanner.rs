#[derive(Debug)]
pub struct Scanner<'src> {
    source: &'src str,
    start: usize,
    current: usize,
    line: u32,
}

impl<'src> Scanner<'src> {
    pub fn new(source_: &'src str) -> Self {
        Self {
            source: source_,
            start: 0,
            current: 0,
            line: 1,
        }
    }

    pub fn scan_token(&mut self) -> Option<Token<'src>> {
        self.skip_whitespace();
        self.start = self.current;

        if self.is_at_end() {
            self.make_token(Kind::EOF);
        }

        let ch: char = self.advance();

        if ch.is_digit(10) {
            return Some(self.number());
        }

        if Self::is_alpha(ch) {
            return Some(self.identifier());
        }

        Some(match ch {
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
            '!' => {
                if self.match_char('=') {
                    self.make_token(Kind::BangEquals)
                } else {
                    self.make_token(Kind::Equal)
                }
            }
            '=' => {
                if self.match_char('=') {
                    self.make_token(Kind::EqualEquals)
                } else {
                    self.make_token(Kind::Equal)
                }
            }
            '<' => {
                if self.match_char('=') {
                    self.make_token(Kind::LessEqual)
                } else {
                    self.make_token(Kind::Less)
                }
            }
            '>' => {
                if self.match_char('=') {
                    self.make_token(Kind::GreaterEqual)
                } else {
                    self.make_token(Kind::Greater)
                }
            }
            '"' => self.string(),
            // _ => todo!(),
            _ => self.error_token("Unexpected character."),
        })
    }

    fn identifier(&mut self) -> Token<'src> {
        while Self::is_alpha(self.peek().unwrap()) || self.peek().unwrap().is_digit(10) {
            self.advance();
        }
        let kind = self.identifier_type();
        self.make_token(kind)
    }

    // associated function: analogous to Java's static methods.
    fn is_alpha(ch: char) -> bool {
        ch == '_' || ch.is_alphabetic()
    }

    fn identifier_type(&mut self) -> Kind {
        // let ch: char = self.source.as_bytes()[self.current] as char;
        // match ch {
        //     'src' => self.check_keyword(1, 2, "nd", Kind::And),
        //     'c' => self.check_keyword(1, 4, "lass", Kind::Class),
        //     'e' => self.check_keyword(1, 3, "lse", Kind::Else),
        //     'i' => self.check_keyword(1, 1, "f", Kind::If),
        //     'n' => self.check_keyword(1, 2, "il", Kind::Nil),
        //     'o' => self.check_keyword(1, 1, "r", Kind::Or),
        //     'p' => self.check_keyword(1, 4, "rint", Kind::Print),
        //     'r' => self.check_keyword(1, 5, "eturn", Kind::Return),
        //     's' => self.check_keyword(1, 4, "uper", Kind::Super),
        //     'v' => self.check_keyword(1, 2, "ar", Kind::Var),
        //     'w' => self.check_keyword(1, 4, "hile", Kind::While),
        //     _ => todo!()
        // }
        // Kind::Identifier
        let text = &self.source[self.start..self.current];
        match text {
            "and" => Kind::And,
            "class" => Kind::Class,
            "else" => Kind::Else,
            "if" => Kind::If,
            "nil" => Kind::Nil,
            "Or" => Kind::Or,
            "print" => Kind::Print,
            "return" => Kind::Return,
            "super" => Kind::Super,
            "var" => Kind::Var,
            "while" => Kind::While,
            _ => Kind::Identifier,
        }
    }

    fn check_keyword(
        &mut self,
        start: usize,
        length: usize,
        rest: &'static str,
        kind: Kind,
    ) -> Kind {
        if self.current - self.start == start + length
            && &self.source[self.start + start..self.start + start + length] == rest
        {
            return kind;
        }
        Kind::Identifier
    }

    fn number(&mut self) -> Token<'src> {
        while self.peek().unwrap().is_digit(10) {
            self.advance();
        }

        if let Some('.') = self.peek() {
            let nxt = self.source.as_bytes()[self.current + 1] as char;
            if nxt.is_digit(10) {
                self.advance(); // consume '.'
                while self.peek().unwrap().is_digit(10) {
                    self.advance();
                }
            }
        }

        self.make_token(Kind::Number)
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
                }
                Some('\n') => {
                    let _ = self.advance();
                    self.line += 1;
                }
                Some('/') => {
                    if let Some('/') = self.source[self.current + 1..].chars().next() {
                        while self.peek().unwrap() != '\n' && !self.is_at_end() {
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

    fn string(&mut self) -> Token<'src> {
        while self.peek().unwrap() != '"' && !self.is_at_end() {
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

    fn make_token(&self, kind: Kind) -> Token<'src> {
        Token {
            kind: kind,
            lexeme: &self.source[self.start..self.current],
            line: self.line,
        }
    }

    // lifetime of Token must be enclosed by msg's i.e
    // msg cannot be 'dropped' before Token.
    // ? Good memory model a combination of C code ::
    // ? char* start and int length is a perfect candidate for a
    // ? &str
    fn error_token(&self, msg: &'static str) -> Token<'src> {
        Token {
            kind: Kind::Error,
            lexeme: msg,
            line: self.line,
        }
    }
}
