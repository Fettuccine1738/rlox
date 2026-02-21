pub mod parser;
pub mod scanner;
pub mod token;

// use std::cell::RefCell;
// use std::rc::Rc;
use std::f64;

use self::parser::Parser;
use self::scanner::Scanner;
use self::token::Kind;
use crate::chunk::Chunk;
use crate::chunk::OpCode;

/// used by ParseRule
type ParseFn = fn(&mut Compiler, bool) -> ();

#[derive(Debug)]
pub struct Compiler<'a> {
    parser: Parser<'a>,
    chunk: &'a  mut Chunk,
}

impl Compiler<'_> {
    pub fn compile(source: &str, chunk: &mut Chunk) -> bool {
        let mut parser: Parser = Parser::new(Scanner::new(source));
        // compiling_chunk may be required later and to allow mulitple owners to mutate
        // chunk. Rc::RefCell is being used here.
        // let _compiling_chunk: Rc<RefCell<&mut Chunk>> = Rc::new(RefCell::new(chunk));
        let mut compiler: Compiler = Compiler { parser, chunk: chunk };
        compiler.parser.advance();
        compiler.expression();
        compiler.consume(Kind::EOF, "Expected end of expression.");
        compiler.end_compilation();

        compiler.parser.had_error
    }

    fn consume(&mut self, kind: Kind, err_msg: &'static str) {
        todo!()
    }

    fn end_compiler(&mut self) {
        self.emit_return();
    }

    fn emit_return(&mut self) {
        self.emit_byte(OpCode::Return as u8);
    }

    // byte may be opcode or operand
    fn emit_byte(&mut self, byte: u8) {
        self.chunk.write(byte, self.parser.previous.line);
    }

    fn emit_bytes(&mut self, byte_1: u8, byte_2: u8) {
       self.chunk.write(byte_1, self.parser.previous.line);
       self.chunk.write(byte_2, self.parser.previous.line);
    }

    fn emit_constant(&mut self, value: f64) {
        let op: OpCode = Self::make_constant(value, self.chunk);
        self.emit_byte(op as u8);
    }
 
    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn end_compilation(&mut self) {
        todo!()
    }
    
    fn number(&mut self) {
        let value: f64 = self.parser.previous.lexeme.parse::<f64>().unwrap();  
        self.emit_constant(value);
    }

    // grouping does not need to emit any byte code. its syntax to insert a 
    // lower-precedence expression where a higher one is expected.
    fn grouping(&mut self) {
        self.expression();
        self.consume(Kind::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self) {
        let operator: Kind = self.parser.previous.kind;
        self.parse_precedence(Precedence::Unary);
        self.expression(); // compile the operand
        // emit the operator instruction
        // NOTE: unary operator is emitted after its operand (expr) because our vm
        // is stack based. we negate what is on the stack. 
        match operator {
            Kind::Minus => {
                self.emit_byte(OpCode::Negate as u8);
            }
            _ => (),
        }
    }

    fn binary(&mut self) {
        let operator: Kind = self.parser.previous.kind;
        let rule: &ParseRule = Self::get_parse_rule(operator);
        self.parse_precedence(Precedence::try_from(rule.precedence as u8 + 1).unwrap());

        match operator {
            Kind::Plus => self.emit_byte(OpCode::Add as u8),
            Kind::Minus => self.emit_byte(OpCode::Subtract as u8),
            Kind::Star => self.emit_byte(OpCode::Multiply as u8),
            Kind::Slash => self.emit_byte(OpCode::Divide as u8),
            _ => (),
        }
    }

    /// calls using precedence ensures only operators have higher precedence are executed.
    /// e.g -a.b + c :: without precedence levels becomes -(a.b + c). Precedence correctly
    /// parses it as (-a.b) + c because Precedenc::Unary > Term.  
    fn parse_precedence(&mut self, precedence: Precedence) {
        self.parser.advance();
        if let Some(prefix) = Self::get_parse_rule(self.parser.previous.kind).prefix {
            prefix(self, false);

            while (precedence as u8) < (Self::get_parse_rule(self.parser.current.kind).precedence as u8) {
                self.parser.advance();
                let infix: ParseFn = Self::get_parse_rule(self.parser.previous.kind).infix.unwrap();
                infix(self, false);
            }
        } else {
            // self.error("Expect expression.");
            todo!()
        }
    }

    /// ---------associated functions------------------
    fn make_constant(value: f64, chunk: &mut Chunk) -> OpCode {
        let index = chunk.add_constant(value);
        if index > std::u8::MAX as usize {
            OpCode::Constant
        } else {
            OpCode::Constant24
        }
    }

    fn get_parse_rule(kind: Kind) -> &'static ParseRule {
        &RULES[(kind as u8) as usize]
    }
}


//-----------------Precedence-------------------
// Lox's precedence levels in order from lowest to highest
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Precedence {
    None = 0,
    Assignment, // =
    Or, // or
    And, // and
    Equality, // ==, !=
    Comparison, // <> <= >=
    Term, // + -
    Factor, // * /   
    Unary, // ! -
    Call, // . ()
    Primary = 10
}

impl TryFrom<u8> for Precedence {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Precedence::None),
            1 => Ok(Precedence::Assignment),
            2 => Ok(Precedence::Or),
            3 => Ok(Precedence::And),
            4 => Ok(Precedence::Equality),
            5 => Ok(Precedence::Comparison),
            6 => Ok(Precedence::Term),
            7 => Ok(Precedence::Factor),
            8 => Ok(Precedence::Unary),
            9 => Ok(Precedence::Call),
            10 => Ok(Precedence::Primary),
            _ => Err(()),
        }
    }
}

// -----------------ParseRule ---------------

#[derive(Debug, Clone, Copy)]
pub struct ParseRule {
    prefix:  Option<ParseFn>, // Option<Box<dyn FnMut(&mut Compiler, bool)>>,
    infix:  Option<ParseFn>, // Option<Box<dyn FnMut(&mut Compiler, bool)>>,
    precedence: Precedence,
}

impl ParseRule {
    const fn new(p_fix: ParseFn, i_fix: ParseFn, precedenc: Precedence) -> Self {
        Self {
            prefix: Some(p_fix),
            infix: Some(i_fix),
            precedence: precedenc
        }
    }

    const fn new_prefix(p_fix: ParseFn, precedenc: Precedence) -> Self {
        Self {
            prefix: Some(p_fix),
            infix: None,
            precedence: precedenc
        }
    }

    const fn new_infix(i_fix: ParseFn, precedenc: Precedence) -> Self {
        Self {
            prefix: None,
            infix: Some(i_fix),
            precedence: precedenc
        }
    }

    const fn new_preceedence(precedenc: Precedence) -> Self {
        Self {
            prefix: None,
            infix: None,
            precedence: precedenc
        }
    }

    const fn default() -> Self {
        Self {
            prefix: None,
            infix: None,
            precedence: Precedence::None
        }
    }
}

static RULES: [ParseRule; 40] = {
    let none = ParseRule::default();
    let mut  rules = [none; 40];

    rules[(Kind::LeftParen as u8) as usize] = ParseRule::new_prefix(|compiler, _| compiler.grouping(), Precedence::None);
    rules[(Kind::Minus as u8) as usize] = ParseRule::new(|compiler, _| compiler.unary(),  
    |compiler, _ | compiler.binary(),     Precedence::Term);
    rules[(Kind::Plus as u8) as usize] = ParseRule::new_infix(|compiler, _| compiler.unary(),  Precedence::Term);
    rules[(Kind::Slash as u8) as usize] = ParseRule::new_infix(|compiler, _| compiler.binary(),  Precedence::Factor);
    rules[(Kind::Star as u8) as usize] = ParseRule::new_infix(|compiler, _| compiler.binary(),  Precedence::Factor);
    rules[(Kind::Number as u8) as usize] = ParseRule::new_prefix(|compiler, _| compiler.number(),  Precedence::Factor);

    rules
};