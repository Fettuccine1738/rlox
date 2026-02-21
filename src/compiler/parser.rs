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
    pub had_error: bool,
    pub panic_mode: bool,
}

impl<'src> Parser<'src> {

    pub fn new(scanner_: Scanner) -> Self {
        Self{
            scanner: scanner_,
            current: Token::default(),
            previous: Token::default(),
            had_error: false,
            panic_mode: false,
        }
    }

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

    fn consume(&mut self, kind: Kind, msg: &'static str) {
        if self.current.kind == kind {
            self.advance();
            return;
        } 
        self.error_at_current(msg);
    }

    // byte may be opcode or operand
    fn emit_byte(&self, byte: u8, chunk: &mut Chunk) {
        chunk.write(byte, self.previous.line);
    }

    fn emit_bytes(&self, byte_1: u8, byte_2: u8, chunk: &mut Chunk) {
        chunk.write(byte_1, self.previous.line);
        chunk.write(byte_2, self.previous.line);
    }

    /// --------------error handling--------------
    fn error_at_current(&mut self, message: &'static str) {
        self.error_at(&self.current.clone(), message);
    }

    fn error(&mut self, message: &'static str) {
        // unbelievable this would not work.
        self.error_at(&self.previous.clone(), message);
        // self.error_at(&self.previous, message);
    }

    fn error_at(&mut self, token: &Token<'src>, message: &str) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
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

