use ::string_interner::symbol::SymbolU32;

use super::parser::Parser;
use super::scanner::Scanner;
use super::token::Kind;
use crate::core::chunk::Chunk;
use crate::compile::token::Token;
use crate::data_structures::interner::{self};
use crate::core::opcode::OpCode;
use crate::core::value::Value;

#[derive(Debug)]
pub struct Local<'src> {
    name: Token<'src>,
    // record the scope depth of the where the local var was declared.
    // Sentinel -1 means this local is uninitialized.
    depth: i32,
}

// the source string should not be 'static because you don't want to require that
// it would mean only compiling string literals baked into the binary,
// not strings read from files or stdin at runtime. Keeping it as a generic
// 'src is the right call: it says "tokens borrow from whatever source string you give me,
// and that source just needs to outlive the compiler".
#[derive(Debug)]
pub struct Compiler<'a, 'src> {
    // 'src annotation here tells Rust that the tokens current and previous
    // in the parser lives as long as the source string.
    parser: Parser<'src>,
    chunk: &'a mut Chunk,
    locals: Vec<Local<'src>>,
    scope_depth: i32, // the number of blocks surrouding the current bit of code being compiled.
                      // local_count: u32 not needed, vec.len() already tracks how many locals are in scope.
}

impl<'src> Compiler<'_, 'src> {
    // associated function, like java static functions
    pub fn compile(source: &str, chunk: &mut Chunk) -> bool {
        let mut compiler: Compiler = Compiler {
            parser: Parser::new(Scanner::new(source)),
            chunk: chunk,
            scope_depth: 0,
            locals: Vec::new(),
        };

        compiler.parser.advance();

        while !compiler.match_token(Kind::EOF) {
            compiler.declaration();
        }

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

    fn emit_opcode(&mut self, op_code: OpCode) {
        self.emit_byte(op_code as u8);
    }

    // byte may be opcode or operand
    fn emit_byte(&mut self, byte: u8) {
        self.chunk.write(byte, self.parser.previous.line);
    }

    fn emit_opcodes(&mut self, op_1: OpCode, op_2: OpCode) {
        self.emit_bytes(op_1 as u8, op_2 as u8);
    }

    fn emit_opcode_operand(&mut self, opcode: OpCode, index: usize) {
        self.emit_opcode(opcode);
        // resolve constant operand
        if index > 255 {
            let (bits0_7, bits8_15, bits16) = Chunk::resolve_index(index);
            self.emit_byte(bits0_7);
            self.emit_byte(bits8_15);
            self.emit_byte(bits16);
        } else {
            self.emit_byte(index as u8);
        }
    }

    fn emit_bytes(&mut self, byte_1: u8, byte_2: u8) {
        self.chunk.write(byte_1, self.parser.previous.line);
        self.chunk.write(byte_2, self.parser.previous.line);
    }

    fn emit_constant(&mut self, value: Value) {
        // emits the opcode and its byte operand (the index of the value in the constants array.)
        let index: usize = self.chunk.add_if_absent(value);
        // this lets us record the index that triggers the use of OpCode::Constant24, where reading 3 bytes
        // must be read to get the index of a constant from the constant pool.
        if self.chunk.index_const24 == std::usize::MAX && index > 255 {
            self.chunk.save_index();
        }

        self.emit_opcode_operand(
            if index > 255 {
                OpCode::Constant24
            } else {
                OpCode::Constant
            },
            index,
        );
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn end_compilation(&mut self) {
        self.emit_return();
        #[cfg(any(test, debug_assertions))] // analogous to a #ifdef block in C
        // custom features could be used too. #[cfg(feature="")]
        println!("{:?}", self.chunk);
        Chunk::disassemble(
            self.current_chunk(),
            if self.parser.had_error {
                "Failed to Compile"
            } else {
                "Compile Successful"
            },
        );
    }

    fn current_chunk(&self) -> &Chunk {
        self.chunk
    }

    fn declaration(&mut self) {
        // if current token is Kind::Var consume the variable's name.
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
        let global: usize = self.parse_variable("Expect variable name.");

        // usecase: this branch decides what the Value in Variable declaration is.
        // case: var a = foo();  == the rhs expression  is evaluated.
        // case:  var a; == this expands to var a = NIL;
        if self.match_token(Kind::Equal) {
            self.expression();
        } else {
            // initialize to Nil.
            self.emit_opcode(OpCode::NIL);
        }

        self.consume(Kind::SemiColon, "Expect ';' after expression.");
        self.define_variable(global);
    }

    fn variable(&mut self, can_assign: bool) {
        self.named_variable(self.parser.previous, can_assign)
    }

    fn named_variable(&mut self, name: Token, can_assign: bool) {
        let (get_op, set_op, arg) = match self.resolve_local(&name) {
            Some(index) => (OpCode::GetLocal, OpCode::SetLocal, index),
            None => {
                let idx: usize = self.identifier_constant(name);
                (OpCode::GetGlobal, OpCode::SetGlobal, idx)
            }
        };

        if can_assign && self.match_token(Kind::Equal) {
            self.expression();
            self.emit_opcode_operand(set_op, arg);
        } else {
            self.emit_opcode_operand(get_op, arg);
        }
    }

    fn resolve_local(&mut self, name: &Token) -> Option<usize> {
        for (idx, local) in self.locals.iter().enumerate().rev() {
            if *name == local.name {
                if local.depth == -1 {
                    self.parser
                        .error("Can't read local variable in its own initializer.");
                }
                return Some(idx);
            }
        }
        None
    }

    fn define_variable(&mut self, global: usize) {
        if self.scope_depth > 0 {
            self.mark_initialized();
            return;
        }
        self.emit_opcode_operand(OpCode::DefineGlobal, global);
    }

    // marks a variable as initialized once it has been defined.
    // Declared = variable is in an uninitialized state.
    // Defined = variable is initialized and availble for use.
    fn mark_initialized(&mut self) {
        let index = self.locals.len() - 1;
        self.locals[index].depth = self.scope_depth;
    }

    fn synchronize(&mut self) {
        self.parser.panic_mode = false;

        while self.parser.current.kind != Kind::EOF {
            if self.parser.previous.kind == Kind::SemiColon {
                return;
            }

            match self.parser.current.kind {
                Kind::Class
                | Kind::Fun
                | Kind::Var
                | Kind::If
                | Kind::While
                | Kind::Print
                | Kind::Return => return,
                _ => (),
            }
        }
    }

    fn statement(&mut self) {
        if self.match_token(Kind::Print) {
            self.print_statement();
        } else if self.match_token(Kind::LeftBrace) {
            self.begin_scope();
            self.block();
            self.end_scope();
        } else if self.match_token(Kind::If) {
            self.if_statement();
        } else if self.match_token(Kind::While) {
            self.while_statement();
        } else if self.match_token(Kind::For) {
            self.for_statement();
        } else {
            self.expr_statement();
        }
    }

    fn for_statement(&mut self) {
        self.begin_scope();
        self.consume(Kind::LeftParen, "Expect '(' after 'for'.");
        if self.match_token(Kind::SemiColon) {
            // no initializer.
        } else if self.match_token(Kind::Var) {
            self.variable_declaration();
        } else {
            self.expr_statement();
        }

        self.consume(Kind::SemiColon, "Expect ';'.");
        let mut loop_start = self.count();
        let mut exit_jump: Option<usize> = None;

        if !self.match_token(Kind::SemiColon) {
            self.expression();
            self.consume(Kind::SemiColon, "Expect ';' after loop condition.");
            // jump out of the loop if the condition is false
            exit_jump = Some(self.emit_jump(OpCode::JumpIfFalse));
            self.emit_opcode(OpCode::Pop);
        }

        self.emit_loop(loop_start);
        match exit_jump {
            Some(jump) => {
                self.patch_jump(jump);
                self.emit_opcode(OpCode::Pop);
            }
            _ => (),
        }

        if !self.match_token(Kind::RightParen) {
            let body_jump = self.emit_jump(OpCode::Jump);
            let increment_start = self.count();
            self.expression();
            self.emit_opcode(OpCode::Pop);
            self.consume(Kind::RightParen, "Expect ')' after for clauses.");
            self.emit_loop(loop_start);

            loop_start = increment_start;
            self.patch_jump(body_jump);
        }
        self.statement();
        self.end_scope();
    }

    fn while_statement(&mut self) {
        let loop_start = self.count(); // jump all the way back to here if condition is true
        self.consume(Kind::LeftParen, "Expect '(' after 'while'.");
        self.expression();
        self.consume(Kind::LeftParen, "Expect ')' after condition.");

        // exit_jump is dummy location to jump to outside the while loop
        // while true { block } (jump here.)
        let exit_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_opcode(OpCode::Pop); // pops the condition of the stack 
        self.statement();
        // backward loop after boy is executed.
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump); // update correct location to goto.
        self.emit_opcode(OpCode::Pop);
    }

    fn emit_loop(&mut self, start: usize) {
        self.emit_opcode(OpCode::Loop);

        let offset = self.count() - start + 2;
        if offset as u16 > std::u16::MAX {
            self.parser.error("Loop body too large.");
        }
        self.emit_byte((offset & 0xff) as u8);
        self.emit_byte(((offset >> 8) & 0xff) as u8);
    }

    fn count(&self) -> usize {
        self.chunk.code.len()
    }

    // NOTE: statments have zero stack effect i.e do not leave values on the stack.
    fn if_statement(&mut self) {
        self.consume(Kind::LeftParen, "Expect '(' after 'if'.");
        self.expression(); // compile the condition expression, leaving it on the stack.
        self.consume(Kind::RightParen, "Expect ')' after 'if'.");

        let then_jump: usize = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_opcode(OpCode::Pop); // if the condition is true, pop before compiling the then block. 
        self.statement();

        // the else jump is unconditional: It jumps to the next stmt after the else branch.
        let else_jump: usize = self.emit_jump(OpCode::Jump);

        self.patch_jump(then_jump);
        self.emit_opcode(OpCode::Pop); // pop the condition before the else branch.
        if self.match_token(Kind::Else) {
            self.statement();
        }
        self.patch_jump(else_jump);
    }

    // the 'lhs' of the expression has been compiled with its value on the stack.
    // if the value is false the entire and must be false and the 'rhs' is skipped.o
    // otherwise we discard the lhs and evaluate the rhs as the result of the whole and expression.
    // (lhs: Value on stack) [OP_JUMP_IF_FALSE, OP_POP] (rhs: not yet compiled.) {end_jump + vm.ip : jumps here after if lhs is false}
    //                      ^(current)         (if value is true: pop lhs off the stack and evaluate rhs as final)
    fn and(&mut self, can_assign: bool) {
        let end_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_opcode(OpCode::Pop);
        self.parse_precedence(Precedence::And);
        self.patch_jump(end_jump);
    }

    fn or(&mut self, can_assign: bool) {
        let else_jump = self.emit_jump(OpCode::JumpIfFalse);
        let end_jump = self.emit_jump(OpCode::Jump);

        self.patch_jump(else_jump);
        self.emit_opcode(OpCode::Pop);

        self.parse_precedence(Precedence::Or);
        self.patch_jump(end_jump);
    }

    // returns the index where the  (operand to the OpCode)
    // which is how much to offset the instruction ptr
    // i.e how many bytes of code to skip.
    fn emit_jump(&mut self, instruction: OpCode) -> usize {
        self.emit_opcode(instruction);
        // placeholder operand for the jump offset.
        // 16-bit offset to jump over 65,535 bytes of code.
        self.emit_byte(0xFF);
        self.emit_byte(0xFF);
        self.count() - 2
    }

    fn patch_jump(&mut self, offset: usize) {
        // - 2 to adjust for the bytecode for the jump offset itself.
        // jump is how many bytecodes have been generated since we consumed the if stmt.
        let jump = self.count() - 2 - offset;

        if jump as u16 > std::u16::MAX {
            self.parser.error("Too much code to jump over.");
        }

        let jump = jump as u32;
        // little-endian
        self.chunk.code[offset] = (jump & 0xFF) as u8;
        self.chunk.code[offset + 1] = (jump >> 8) as u8;
    }

    fn block(&mut self) {
        while !self.check(Kind::RightBrace) && !self.check(Kind::EOF) {
            self.declaration();
        }
        self.consume(Kind::RightBrace, "Expect '}' after block.");
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.scope_depth -= 1;

        while !self.locals.is_empty() && self.locals[self.locals.len() - 1].depth > self.scope_depth
        {
            self.emit_opcode(OpCode::Pop);
            self.locals.pop(); // discard this value.
        }
    }

    fn expr_statement(&mut self) {
        self.expression();
        self.consume(Kind::SemiColon, "Expect ';' after expression.");
        self.emit_opcode(OpCode::Pop);
    }

    fn print_statement(&mut self) {
        self.expression(); // evaluates expression to print sstatement.
        self.consume(Kind::SemiColon, "Expect ';' after value.");
        self.emit_opcode(OpCode::Print);
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
            Kind::BangEquals => self.emit_opcodes(OpCode::Equal, OpCode::Not),
            Kind::EqualEquals => self.emit_opcode(OpCode::Equal),
            Kind::Greater => self.emit_opcode(OpCode::Greater),
            Kind::GreaterEqual => self.emit_opcodes(OpCode::Less, OpCode::Not),
            Kind::Less => self.emit_opcode(OpCode::Less),
            Kind::LessEqual => self.emit_opcodes(OpCode::Greater, OpCode::Not),
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
        let lexeme = self.parser.previous.lexeme;
        // trim off "" from both ends
        let trimmed_lexeme = &lexeme[1..lexeme.len() - 1];
        self.emit_constant(Value::String(interner::intern(trimmed_lexeme)));
    }

    /// calls using precedence ensures only operators have higher precedence are executed.
    /// e.g -a.b + c :: without precedence levels becomes -(a.b + c). Precedence correctly
    /// parses it as (-a.b) + c because Precedenc::Unary > Term.  
    fn parse_precedence(&mut self, precedence: Precedence) {
        self.parser.advance();
        if let Some(prefix) = Self::get_parse_rule(self.parser.previous.kind).prefix {
            // Some infix operations destroy the precedence operation.
            // take a * b = c + d;
            // when parsing the right hand to the infix op(*), variable b
            // accepts any Precedent(None) and the expression expands to
            // a * (b = c + d) instead.
            // Can-assign; allows assignement when in an assignment expression or top-level expression e.g expr-stmt.
            let can_assign: bool = precedence <= Precedence::Assignment;
            prefix(self, can_assign);

            let dbg_prec = precedence as u8;

            while dbg_prec <= (Self::get_parse_rule(self.parser.current.kind).precedence as u8) {
                self.parser.advance();
                match Self::get_parse_rule(self.parser.previous.kind).infix {
                    Some(infix) => infix(self, can_assign),
                    None => self.parser.error("Unexpected infix expression."),
                }
            }

            if can_assign && self.match_token(Kind::Equal) {
                self.parser.error("Invalid assignment target.");
            }
        } else {
            self.parser.error("expected an expression here.");
        }
    }

    /// consumes the identifier token for the variable name, adds its lexeme
    /// to the chunk’s constant table as a string, and then returns the
    /// constant table index where it was added
    fn parse_variable(&mut self, err_msg: &'static str) -> usize {
        self.consume(Kind::Identifier, err_msg);
        self.declare_variable();
        if self.scope_depth > 0 {
            // exit the function if in a local scope.
            return 0;
        }
        self.identifier_constant(self.parser.previous)
    }

    fn declare_variable(&mut self) {
        if self.scope_depth == 0 {
            return;
        }
        let name = self.parser.previous;

        // NOTE: search direction here is required to search most
        // recent declarations.
        for local in self.locals.iter().rev() {
            // local.depth < scope depth means the current local is an outer variable
            // stops search.
            if local.depth != -1 && local.depth < self.scope_depth {
                break;
            }

            if name.lexeme == local.name.lexeme {
                self.parser
                    .error("Variable with this name exists in this scope.");
            }
        }
        self.add_local(name);
    }

    fn add_local(&mut self, token: Token<'src>) {
        self.locals.push(Local {
            name: token,
            depth: self.scope_depth,
        });
    }

    fn identifier_constant(&mut self, token: Token) -> usize {
        let symbol: SymbolU32 = interner::intern(token.lexeme);
        self.chunk.add_if_absent(Value::String(symbol))
    }

    fn get_parse_rule(kind: Kind) -> &'static ParseRule {
        &RULES[(kind as u8) as usize]
    }
}

//-----------------Precedence-------------------
// Lox's precedence levels in order from lowest to highest
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
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
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Term);
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
        ParseRule::new_prefix(|compiler, _| compiler.number(), Precedence::None);

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
    rules[(Kind::Identifier as u8) as usize] = ParseRule::new_prefix(
        |compiler, can_assign| compiler.variable(can_assign),
        Precedence::None,
    );
    rules[(Kind::And as u8) as usize] = ParseRule::new_infix(
        |compiler, can_assign| compiler.and(can_assign),
        Precedence::And,
    );
    rules[(Kind::Or as u8) as usize] = ParseRule::new_infix(
        |compiler, can_assign| compiler.and(can_assign),
        Precedence::Or,
    );
    rules
};