pub mod parser;
pub mod scanner;
pub mod token;

use std::cell::RefCell;
use std::rc::Rc;

use crate::chunk::Chunk;
use self::token::Token;
use self::parser::Parser;
use self::scanner::Scanner;

pub struct Compiler {
}

pub fn compile(source: &str, chunk: &mut Chunk) -> bool {
    let mut parser: Parser = Parser::new(Scanner::new(source));
    // compiling_chunk may be required later and to allow mulitple owners to mutate
    // chunk. Rc::RefCell is being used here.
    let compiling_chunk: Rc<RefCell<&mut Chunk>> = Rc::new(RefCell::new(chunk));
    parser.advance();
    expression();
    consume(Kind::EOF, "Expected end of expression.");
    end_compiler();

    parser.had_error
}


fn end_compiler() {
    emit_return();
}

fn emit_return(parser: &mut Parser, chunk: &mut Chunk) {
    parser.emit_byte(OpCode::Return as u8, chunk);
}

fn expression() {
    todo!()
}
