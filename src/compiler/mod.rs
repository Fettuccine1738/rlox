pub mod parser;
pub mod scanner;
pub mod token;

use ::string_interner::symbol::SymbolU32;

use self::parser::Parser;
use self::scanner::Scanner;
use self::token::Kind;
use crate::chunk::Chunk;
use crate::chunk::OpCode;
use crate::value::Value;
use crate::compiler::token::Token;
use crate::data_structures::interner::{self};

#[derive(Debug)]
pub struct Compiler<'a> {
    parser: Parser<'a>,
    chunk: &'a mut Chunk,
}

impl Compiler<'_> {
    // associated function, like java static functions
    pub fn compile(source: &str, chunk: &mut Chunk) -> bool {
        let mut compiler: Compiler = Compiler {
            parser: Parser::new(Scanner::new(source)),
            chunk: chunk,
        };
        compiler.parser.advance();

        while !compiler.match_token(Kind::EOF) {
            compiler.declaration();
        }
        // compiler.expression();
        // compiler.consume(Kind::EOF, "Expected end of expression.");

        compiler.end_compilation();
        !compiler.parser.had_error
    }

    fn consume(&mut self, kind: Kind, err_msg: &'static str) {
        self.parser.consume(kind, err_msg);
    }

    fn match_token(&mut self, kind: Kind) -> bool {
        if !self.check(kind) {
            false
        } else {
            self.parser.advance();
            true
        }
    }

    fn check(&mut self, kind: Kind) -> bool {
        self.parser.current.kind == kind
    }

    fn emit_return(&mut self) {
        self.emit_byte(OpCode::Return as u8);
    }

    fn emit_op_code_byte(&mut self, op_code: OpCode) {
        self.emit_byte(op_code as u8);
    }

    // byte may be opcode or operand
    fn emit_byte(&mut self, byte: u8) {
        self.chunk.write(byte, self.parser.previous.line);
    }

    fn emit_op_code_bytes(&mut self, op_1: OpCode, op_2: OpCode) {
        self.emit_bytes(op_1 as u8, op_2 as u8);
    }

    fn emit_bytes(&mut self, byte_1: u8, byte_2: u8) {
        self.chunk.write(byte_1, self.parser.previous.line);
        self.chunk.write(byte_2, self.parser.previous.line);
    }

    fn emit_constant(&mut self, value: Value) {
        let op: OpCode = Self::make_constant(value, self.chunk);
        self.emit_byte(op as u8);
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn end_compilation(&mut self) {
        self.emit_return();
        #[cfg(debug_assertions)] // analogous to a #ifdef block in C
        // custom features could be used too. #[cfg(feature="")]
        if self.parser.had_error {
            Chunk::disassemble(self.current_chunk(), "code");
        }
    }

    fn current_chunk(&self) -> &Chunk {
        self.chunk
    }

    fn declaration(&mut self) {
        if self.match_token(Kind::Var) {
            self.variable_declaration();
        } else {
            self.statement();
        }

        if self.parser.panic_mode {
            self.synchronize();
        }
    }

    fn variable_declaration(&mut self) {
        let global: OpCode = self.parse_variable("Expect variable name.");

        // usecase: this branch decides what the Value in Variable declaration is.
        // case: var a = foo();  == the rhs expression  is evaluated.
        // case:  var a; == this expands to var a = NIL;
        if self.match_token(Kind::Equal) {
            self.expression();
        } else { // initialize to Nil.
            self.emit_op_code_byte(OpCode::NIL);
        }

        self.consume(Kind::SemiColon, "Expect ';' after expression.");
        self.define_variable(global);
    }

    fn variable(&mut self) {
        self.named_variable(self.parser.previous)
    }

    fn named_variable(&mut self, token: Token) {
        let op_code: OpCode = self.identifier_constant(token);
        self.emit_op_code_bytes(OpCode::GetGlobal, op_code);
    }

    fn define_variable(&mut self, global: OpCode) {
        let dummy = OpCode::Add; //NOTE: remove later.
        self.emit_op_code_bytes(dummy, global);
    }

    fn synchronize(&mut self) {
        self.parser.panic_mode = false;

        while self.parser.current.kind != Kind::EOF {
            if self.parser.previous.kind == Kind::SemiColon {
                return;
            }

            match self.parser.current.kind {
                Kind::Class | Kind::Fun | Kind::Var | Kind::If |
                Kind::While | Kind::Print | Kind::Return => return,
                _ => (),
            }
        }
    }

    fn statement(&mut self) {
        if self.match_token(Kind::Print) {
            self.print_statement();
        } else {
            self.expr_statement();
        }
    }

    fn expr_statement(&mut self) {
        self.expression();
        self.consume(Kind::SemiColon, "Expect ';' after expression.");
        self.emit_op_code_byte(OpCode::Pop);
    }

    fn print_statement(&mut self) {
        self.expression(); // evaluates expression to print sstatement.
        self.consume(Kind::SemiColon, "Expect ';' after value.");
        self.emit_op_code_byte(OpCode::Print);
    }

    fn number(&mut self) {
        let value: f64 = self.parser.previous.lexeme.parse::<f64>().unwrap();
        self.emit_constant(Value::Number(value));
    }

    // grouping does not need to emit any byte code. its syntax to insert a
    // lower-precedence expression where a higher one is expected.
    fn grouping(&mut self) {
        self.expression();
        self.consume(Kind::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self) {
        let operator: Kind = self.parser.previous.kind;
        // compile the operand
        self.parse_precedence(Precedence::Unary);

        // emit the operator instruction
        // NOTE: unary operator is emitted after its operand (expr) because our vm
        // is stack based. we negate what is on the stack.
        match operator {
            Kind::Minus => {
                self.emit_byte(OpCode::Negate as u8);
            }
            Kind::Bang => {
                self.emit_byte(OpCode::Not as u8);
            }
            _ => (),
        }
    }

    fn binary(&mut self) {
        let operator: Kind = self.parser.previous.kind;
        let rule: &ParseRule = Self::get_parse_rule(operator);
        self.parse_precedence(Precedence::try_from(rule.precedence as u8 + 1).unwrap()); // tries to parse rhs with a higher precedence.

        match operator {
            Kind::Plus => self.emit_byte(OpCode::Add as u8),
            Kind::Minus => self.emit_byte(OpCode::Subtract as u8),
            Kind::Star => self.emit_byte(OpCode::Multiply as u8),
            Kind::Slash => self.emit_byte(OpCode::Divide as u8),
            Kind::BangEquals => self.emit_op_code_bytes(OpCode::Equal, OpCode::Not),
            Kind::EqualEquals => self.emit_op_code_byte(OpCode::Equal),
            Kind::Greater => self.emit_op_code_byte(OpCode::Greater),
            Kind::GreaterEqual => self.emit_op_code_bytes(OpCode::Less, OpCode::Not),
            Kind::Less => self.emit_op_code_byte(OpCode::Less),
            Kind::LessEqual => self.emit_op_code_bytes(OpCode::Greater, OpCode::Not),
            _ => (),
        }
    }

    fn literal(&mut self) {
        match self.parser.previous.kind {
            Kind::False => self.emit_byte(OpCode::False as u8),
            Kind::True => self.emit_byte(OpCode::True as u8),
            Kind::Nil => self.emit_byte(OpCode::NIL as u8),
            _ => (),
        }
    }

    fn string(&mut self) {
        // trim off "" from both ends
        let lexeme = self.parser.previous.lexeme;
        let trimmed_lexeme = (&lexeme[1..lexeme.len() - 1]).to_owned();
        self.emit_constant(Value::new_string_obj(trimmed_lexeme));
        // NOTE: New method to creating and working with string.
        // let symbol = StringInterner.get_or_intern(trimmed_lexeme);
        // self.emit_constant(Value::String(symbol.to_usize()));
    }

    /// calls using precedence ensures only operators have higher precedence are executed.
    /// e.g -a.b + c :: without precedence levels becomes -(a.b + c). Precedence correctly
    /// parses it as (-a.b) + c because Precedenc::Unary > Term.  
    fn parse_precedence(&mut self, precedence: Precedence) {
        self.parser.advance();
        if let Some(prefix) = Self::get_parse_rule(self.parser.previous.kind).prefix {
            prefix(self, false);

            let dbg_prec = precedence as u8;

            while dbg_prec <= (Self::get_parse_rule(self.parser.current.kind).precedence as u8) {
                self.parser.advance();
                let infix: ParseFn = Self::get_parse_rule(self.parser.previous.kind)
                    .infix
                    .unwrap();
                infix(self, false);
            }
        } else {
            self.parser.error("expected an expression here.");
        }
    }

    fn parse_variable(&mut self, err_msg: &'static str) -> OpCode {
        self.consume(Kind::Identifier, err_msg);
        self.identifier_constant(self.parser.previous)
    }

    // code smell: Does this really need to be an associated func?? 
    fn identifier_constant(&mut self, token: Token) -> OpCode {
        let symbol: SymbolU32 = interner::intern(token.lexeme);
        Self::make_constant(Value::String(symbol), self.chunk)
    }

    /// ---------associated functions------------------
    fn make_constant(value: Value, chunk: &mut Chunk) -> OpCode {
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
    Assignment = 1, // =
    Or = 2,         // or
    And = 3,        // and
    Equality = 4,   // ==, !=
    Comparison = 5, // <> <= >=
    Term = 6,       // + -
    Factor = 7,     // * /
    Unary = 8,      // ! -
    Call = 9,       // . ()
    Primary = 10,
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
type ParseFn = fn(&mut Compiler, bool) -> ();
// type ParseFn = fn(&mut Compiler, bool) -> ();
#[derive(Debug, Clone, Copy)]
pub struct ParseRule {
    prefix: Option<ParseFn>, // Option<Box<dyn FnMut(&mut Compiler, bool)>>,
    infix: Option<ParseFn>,  // Option<Box<dyn FnMut(&mut Compiler, bool)>>,
    precedence: Precedence,
}

impl ParseRule {
    /// const fn because this functions are called by the static RULES block below
    /// this allows the compiler to call this functions at compile time.
    const fn new(p_fix: ParseFn, i_fix: ParseFn, precedenc: Precedence) -> Self {
        Self {
            prefix: Some(p_fix),
            infix: Some(i_fix),
            precedence: precedenc,
        }
    }

    const fn new_prefix(p_fix: ParseFn, precedenc: Precedence) -> Self {
        Self {
            prefix: Some(p_fix),
            infix: None,
            precedence: precedenc,
        }
    }

    const fn new_infix(i_fix: ParseFn, precedenc: Precedence) -> Self {
        Self {
            prefix: None,
            infix: Some(i_fix),
            precedence: precedenc,
        }
    }

    const fn new_precedence(precedenc: Precedence) -> Self {
        Self {
            prefix: None,
            infix: None,
            precedence: precedenc,
        }
    }

    const fn default() -> Self {
        Self {
            prefix: None,
            infix: None,
            precedence: Precedence::None,
        }
    }
}

// The Pratt Parser decides how much of the expression to consume when parsing the right-hand side (RHS)
// of a binary operator.
static RULES: [ParseRule; 40] = {
    let default = ParseRule::default();
    let mut rules = [default; 40];

    rules[(Kind::LeftParen as u8) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.grouping(), Precedence::None);
    rules[(Kind::Minus as u8) as usize] = ParseRule::new(
        |compiler, _| compiler.unary(),
        |compiler, _| compiler.binary(),
        Precedence::Term,
    );

    rules[(Kind::Plus as u8) as usize] =
        ParseRule::new_infix(|compiler, _| compiler.unary(), Precedence::Term);
    rules[(Kind::Slash as u8) as usize] =
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Factor);
    rules[(Kind::Star as u8) as usize] =
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Factor);
    rules[(Kind::True as u8) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.literal(), Precedence::None);
    rules[(Kind::False as u8) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.literal(), Precedence::None);
    rules[(Kind::Nil as u8) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.literal(), Precedence::None);
    rules[(Kind::Number as u8) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.literal(), Precedence::None);

    rules[(Kind::Bang as u8) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.unary(), Precedence::None);
    rules[(Kind::BangEquals as u8) as usize] =
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Equality);
    rules[(Kind::EqualEquals as u8) as usize] =
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Equality);
    rules[(Kind::GreaterEqual as u8) as usize] =
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Comparison);
    rules[(Kind::LessEqual as u8) as usize] =
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Comparison);
    rules[(Kind::Less as u8) as usize] =
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Comparison);
    rules[(Kind::Greater as u8) as usize] =
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Comparison);
    rules[(Kind::String as u8) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.string(), Precedence::None);
    rules[(Kind::Identifier as u8) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.variable(), Precedence::None);
    rules
};
