use std::mem;

use crate::chunk::Chunk;
use crate::scanner::Kind;
use crate::scanner::Scanner;
use crate::scanner::Token;

#[derive(Debug)]
pub struct Parser<'src> {
    /// 'src is the lifetime of the source string slices stored in tokens and
    /// 'scn is the shorter lifetime of the mutable borrow of the scanner itself.
    /// This allow Rust to know the scanner borrow can end indpendently of how long
    /// the token string slices live.
    scanner: Scanner<'src>,
    current: Token<'src>,
    previous: Token<'src>,
    had_error: bool,
    panic_mode: bool,
}

impl<'src> Parser<'src> {
    pub fn advance(&mut self) {
        loop {
            let temp: Token<'_> = self.scanner.scan_token().unwrap();
            // mem::replace returns old value of mutable ref of destination and initializes with new value temp.
            self.previous = mem::replace(&mut self.current, temp);

            if self.current.kind != Kind::Error {
                break;
            }
            self.error_at_current("Scanner error.");
        }
    }

    fn error_at_current(&mut self, message: &'static str) {
        self.error_at(&self.current.clone(), message);
    }

    fn error(&mut self, message: &'static str) {
        // unbelievable this would not work.
        self.error_at(&self.previous.clone(), message);
        // self.error_at(&self.previous, message);
    }

    fn error_at(&mut self, token: &Token<'src>, message: &str) {
        eprint!("[line {}] Error", token.line);
        match token.kind {
            Kind::EOF => eprint!(" at the end"),
            Kind::Error => todo!(),
            _ => eprint!("  at  {}", token.lexeme),
        }
        eprintln!(" : {}", message);
        self.had_error = true;
    }
}

pub fn compile(source: &str, chunk: &mut Chunk) -> bool {
    let mut scanner = Scanner::new(source);
    let mut parser: Parser = todo!();

    advance(scanner);
    expression();
    consume(Kind::EOF, "Expected end of expression.");
    todo!()
}

fn expression() {
    todo!()
}

fn consume(kind: Kind, msg: &str) {
    todo!()
}
