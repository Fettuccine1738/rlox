use std::cell::RefCell;
use std::rc::Rc;
use std::{mem, usize};

use super::parser::Parser;
use super::token::Kind;
use crate::compile::token::Token;
use crate::core::chunk::Chunk;
use crate::core::opcode::OpCode;
use crate::core::{lang::Function, lang::FunctionType, value::Value};
use crate::data_structures::interner::{self, intern};

pub const FUNCTION_ARG_MAX: u8 = 255;

#[derive(Debug, Default)]
pub struct Local<'src> {
    name: Token<'src>,
    // record the scope depth of the where the local var was declared.
    // Sentinel -1 means this local is uninitialized.
    depth: i32,
    is_const: bool,
}

// NOTE: when to name the lifetime when creating impl blocks.
// when returning a reference from a method that requires a lifetime e.g
// pub fn name(&self) -> &'src str {
//     self.name.lexeme
// } else it is fine to just use an anonymous lifetime <'_>
// when methods don't take an argument/return reference related to src
impl Local<'_> {
    pub fn is_initialized(&self) -> bool {
        self.depth != -1
    }

    pub fn is_immutable(&self) -> bool {
        self.is_const
    }
}

// the source string should not be 'static because you don't want to require that
// it would mean only compiling string literals baked into the binary,
// not strings read from files or stdin at runtime. Keeping it as a generic
// 'src is the right call: it says "tokens borrow from whatever source string you give me,
// and that source just needs to outlive the compiler".
#[derive(Debug, Default)]
pub struct Compiler<'src> {
    // 'src annotation here tells Rust that the tokens current and previous
    // in the parser lives as long as the source string.
    parser: Rc<RefCell<Parser<'src>>>,
    locals: Vec<Local<'src>>,
    const_globals: Vec<usize>,
    scope_depth: i32, // the number of blocks surrouding the current bit of code being compiled.
    // local_count: u32 not needed, vec.len() already tracks how many locals are in scope.
    function: Function,
    function_type: FunctionType,
    enclosing: Option<Box<Compiler<'src>>>,
}

impl<'src> Compiler<'src> {
    // associated function, like java static functions
    /// The VM passes a Chunk to the compiler which it fills with code.
    /// now the compiler will create and return a function that contains the
    /// compiled top-level code.
    pub fn compile(source: &str) -> Option<Rc<Function>> {
        let mut compiler: Compiler = Compiler {
            // NOTE: parser is enclosed here for interior mutability. when compiling functions,
            // reference to the outer parser is needed to continue the single pass.
            parser: Rc::new(RefCell::new(Parser::new(source))),
            scope_depth: 0,
            locals: Vec::new(),
            const_globals: Vec::new(),
            // interior mutabliity, this is so we can return the function after compiling
            // and don't have to worry about `dangling` ptr once compile is finished.
            function: Function::new(),
            function_type: FunctionType::default(),
            enclosing: None,
        };

        // why do we need this??
        compiler.locals.push(Local {
            name: Token::default(),
            depth: 0,
            is_const: false,
        });

        compiler.parser.borrow_mut().advance();

        while !compiler.match_token(Kind::EOF) {
            compiler.declaration();
        }

        // Rc<RefCell<T> allows 'interior mutability' = Multiple owners who can all mutate
        // RefCell alone — one owner, mutable. Fine, but...
        // let a = RefCell::new(String::from("hello"));
        // let b = a;  // ❌ moved — a is gone. Only one owner.

        // Rc alone — multiple owners, but...
        // let a = Rc::new(String::from("hello"));
        // let b = Rc::clone(&a);
        // a.push_str(" world");  // ❌ can't mutate through Rc
        let function: Rc<Function> = compiler.end_compilation();
        if compiler.parser.borrow().had_error {
            None
        } else {
            Some(function)
        }
    }

    fn consume(&mut self, kind: Kind, err_msg: &'static str) {
        self.parser.borrow_mut().consume(kind, err_msg);
    }

    fn match_token(&mut self, kind: Kind) -> bool {
        if !self.check(kind) {
            false
        } else {
            self.parser.borrow_mut().advance();
            true
        }
    }

    fn check(&mut self, kind: Kind) -> bool {
        self.parser.borrow().current.kind == kind
    }

    fn emit_return(&mut self) {
        self.emit_opcode(OpCode::NIL);
        self.emit_opcode(OpCode::Return);
    }

    fn emit_opcode(&mut self, op_code: OpCode) {
        self.emit_byte(op_code as u8);
    }

    // byte may be opcode or operand
    fn emit_byte(&mut self, byte: u8) {
        let line = self.parser.borrow().previous.line;
        self.current_chunk().write(byte, line);
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
        let line = self.parser.borrow().previous.line;
        self.current_chunk().write(byte_1, line);
        self.current_chunk().write(byte_2, line);
    }

    fn emit_constant(&mut self, value: Value) {
        // emits the opcode and its byte operand (the index of the value in the constants array.)
        let index: usize = self.current_chunk().add_if_absent(value);
        // this lets us record the index that triggers the use of OpCode::Constant24, where reading 3 bytes
        // must be read to get the index of a constant from the constant pool.
        if self.current_chunk().index_const24 == std::usize::MAX && index > 255 {
            self.current_chunk().save_index();
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

    fn end_compilation(&mut self) -> Rc<Function> {
        self.emit_return();
        #[cfg(any(test, debug_assertions))] // analogous to a #ifdef block in C
        // custom features could be used too. #[cfg(feature="")]
        // println!("{:?}", self.function);
        let name = self
            .function
            .name
            .as_deref()
            .unwrap_or("Script")
            .to_string();
        let status = if self.parser.borrow().had_error {
            "Failed to Compile"
        } else {
            "Compile successful"
        };
        let display_string = format!("{}  :  {}", name, status);

        Chunk::disassemble(self.current_chunk(), &display_string);

        let function = std::mem::take(&mut self.function);
        Rc::new(function)
    }

    /// the current chunk is always the chunk owned by the function currently
    /// being compiled.
    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.function.chunk
    }

    fn declaration(&mut self) {
        // if current token is Kind::Var consume the variable's (lexeme) name.
        if self.match_token(Kind::Fun) {
            self.func_declaration();
        } else if self.match_token(Kind::Var) {
            self.variable_declaration(false);
        } else if self.match_token(Kind::Const) {
            self.variable_declaration(true);
        } else if self.match_token(Kind::Return) {
            self.return_statement();
        } else {
            self.statement();
        }

        if self.parser.borrow().panic_mode {
            self.synchronize();
        }
    }

    fn return_statement(&mut self) {
        if self.function_type == FunctionType::Script {
            self.parser
                .borrow_mut()
                .error("Can't return from top-level code.");
        }

        if self.match_token(Kind::SemiColon) {
            // if there is no return value, implictly return NIL
            self.emit_return();
        } else {
            self.expression();
            self.consume(Kind::SemiColon, "Expect ';' after return value.");
            self.emit_opcode(OpCode::Return);
        }
    }

    fn func_declaration(&mut self) {
        // top level function declaration at the top level binds to a global variable.
        // NOTE: is_const = false here because functions are implicitly immutable.
        let global: usize = self.parse_variable("Expect function name.", false);
        // mark_initialized was used to prevent using a variable before it is fully defined.
        // i.e var foo = foo; . However we don't want this restriction on function.
        // e.g for recursion. so we immediately mark it as initialized before compiling the function body.
        self.mark_initialized(false);
        self.function(FunctionType::Function);
        self.define_variable(global, false);
    }

    fn function(&mut self, func_type: FunctionType) {
        // we take out self because of weird lifetime issues and replace with default
        // enclosing is returned back into self.
        let mut enclosing = std::mem::take(self);
        let function_name = enclosing.parser.borrow().previous.lexeme;
        enclosing.consume(Kind::LeftParen, "Expect '(' in function declaration.");

        let mut inner: Compiler = Compiler {
            parser: enclosing.parser.clone(),
            locals: Vec::new(),
            scope_depth: enclosing.scope_depth,
            const_globals: Vec::new(),
            function: Function::new(),
            function_type: func_type,
            enclosing: Some(Box::new(enclosing)),
        };

        inner.function.name = Some(function_name.to_owned());

        if !inner.check(Kind::RightParen) {
            loop {
                inner.function.arity += 1;
                if inner.function.arity > FUNCTION_ARG_MAX {
                    inner
                        .parser
                        .borrow_mut()
                        .error_at_current("Function cannot have more than 255 parameters.");
                }
                // we probably should also make is_const true at some point and force unique function names.
                let constant = inner.parse_variable("Expect parameter name", false);
                inner.define_variable(constant, false);
                if !self.match_token(Kind::Comma) {
                    break;
                }
            }
        }

        inner.consume(Kind::RightParen, "Expect ')' after parameters.");
        inner.begin_scope();
        inner.consume(Kind::LeftBrace, "Expect '{' before function body.");
        inner.block();
        let function: Rc<Function> = inner.end_compilation();
        let _inner: Compiler = mem::replace(self, *inner.enclosing.unwrap());
        let index: usize = self
            .current_chunk()
            .add_if_absent(Value::LoxFunction(function));
        self.emit_opcode_operand(
            if index > 255 {
                OpCode::Constant24
            } else {
                OpCode::Constant
            },
            index,
        );
    }

    fn call(&mut self) {
        let arg_count = self.argument_list();
        self.emit_opcode_operand(OpCode::Call, arg_count);
    }

    fn argument_list(&mut self) -> usize {
        let mut arg_count: usize = 0;
        if !self.check(Kind::RightParen) {
            loop {
                self.expression();
                if arg_count == FUNCTION_ARG_MAX as usize {
                    self.parser
                        .borrow_mut()
                        .error("Can't have more than 255 arguments.");
                }
                arg_count += 1;
                if !self.match_token(Kind::Comma) {
                    break;
                }
            }
        }
        self.consume(Kind::RightParen, "Expect ')' after arguments.");
        arg_count
    }

    fn variable_declaration(&mut self, is_const: bool) {
        let global: usize = self.parse_variable("Expect variable name.", is_const);

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
        self.define_variable(global, is_const);
    }

    fn variable(&mut self, can_assign: bool) {
        let name_token = self.parser.borrow().previous;
        self.named_variable(name_token, can_assign)
    }

    fn named_variable(&mut self, name: Token, can_assign: bool) {
        let (get_op, set_op, arg, is_const) = match self.resolve_local(&name) {
            Some(index) => (
                OpCode::GetLocal,
                OpCode::SetLocal,
                index,
                self.locals[index].is_const,
            ),
            None => {
                let idx: usize = self.identifier_constant(name);
                (
                    OpCode::GetGlobal,
                    OpCode::SetGlobal,
                    idx,
                    self.const_globals.contains(&idx),
                )
            }
        };

        if can_assign && self.match_token(Kind::Equal) {
            // compile time check that this slots in the constants pool is immutable
            if is_const {
                let msg = format!("Const variable `{}` cannot be assigned to.", name.lexeme);
                self.parser.borrow_mut().error(&msg);
                return;
            }
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
                        .borrow_mut()
                        .error("Can't read local variable in its own initializer.");
                }
                return Some(idx);
            }
        }
        None
    }

    fn define_variable(&mut self, global: usize, is_const: bool) {
        if self.scope_depth > 0 {
            // local scope.
            self.mark_initialized(is_const);
            return;
        }
        if is_const {
            self.emit_opcode_operand(OpCode::ConstGlobal, global);
            self.const_globals.push(global); // compiler knows index at this slot is immutable
        } else {
            self.emit_opcode_operand(OpCode::DefineGlobal, global);
        }
    }

    // marks a variable as initialized once it has been defined.
    // Declared = variable is in an uninitialized state.
    // Defined = variable is initialized and availble for use.
    fn mark_initialized(&mut self, is_const: bool) {
        if self.scope_depth == 0 {
            return;
        }
        if let Some(local) = self.locals.last_mut() {
            local.depth = self.scope_depth;
            local.is_const = is_const;
        }
    }

    fn synchronize(&mut self) {
        self.parser.borrow_mut().panic_mode = false;

        while self.parser.borrow().current.kind != Kind::EOF {
            if self.parser.borrow().previous.kind == Kind::SemiColon {
                return;
            }

            match self.parser.borrow().current.kind {
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
            self.variable_declaration(false);
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
            self.parser.borrow_mut().error("Loop body too large.");
        }
        self.emit_byte((offset & 0xff) as u8);
        self.emit_byte(((offset >> 8) & 0xff) as u8);
    }

    fn count(&self) -> usize {
        self.function.chunk.code.len()
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
            self.parser
                .borrow_mut()
                .error("Too much code to jump over.");
        }

        let jump = jump as u32;
        // little-endian
        self.current_chunk().code[offset] = (jump & 0xFF) as u8;
        self.current_chunk().code[offset + 1] = (jump >> 8) as u8;
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
        let value: f64 = self.parser.borrow().previous.lexeme.parse::<f64>().unwrap();
        self.emit_constant(Value::Number(value));
    }

    // grouping does not need to emit any byte code. its syntax to insert a
    // lower-precedence expression where a higher one is expected.
    fn grouping(&mut self) {
        self.expression();
        self.consume(Kind::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self) {
        let operator: Kind = self.parser.borrow().previous.kind;
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
        let operator: Kind = self.parser.borrow().previous.kind;
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
        let token_kind = self.parser.borrow().previous.kind;
        match token_kind {
            Kind::False => self.emit_byte(OpCode::False as u8),
            Kind::True => self.emit_byte(OpCode::True as u8),
            Kind::Nil => self.emit_byte(OpCode::NIL as u8),
            _ => (),
        }
    }

    fn string(&mut self) {
        let lexeme = self.parser.borrow().previous.lexeme;
        // trim off "" from both ends
        let trimmed_lexeme = &lexeme[1..lexeme.len() - 1];
        self.emit_constant(Value::String(interner::intern(trimmed_lexeme)));
    }

    /// calls using precedence ensures only operators have higher precedence are executed.
    /// e.g -a.b + c :: without precedence levels becomes -(a.b + c). Precedence correctly
    /// parses it as (-a.b) + c because Precedenc::Unary > Term.  
    fn parse_precedence(&mut self, precedence: Precedence) {
        self.parser.borrow_mut().advance();
        let previous_token = self.parser.borrow().previous;
        if let Some(prefix) = Self::get_parse_rule(previous_token.kind).prefix {
            // Some infix operations destroy the precedence operation.
            // take a * b = c + d;
            // when parsing the right hand to the infix op(*), variable b
            // accepts any Precedent(None) and the expression expands to
            // a * (b = c + d) instead.
            // Can-assign; allows assignement when in an assignment expression or top-level expression e.g expr-stmt.
            let can_assign: bool = precedence <= Precedence::Assignment;
            prefix(self, can_assign);

            let dbg_prec = precedence as u8;

            while dbg_prec
                <= (Self::get_parse_rule(self.parser.borrow().current.kind).precedence as u8)
            {
                self.parser.borrow_mut().advance();
                let token_kind = self.parser.borrow().previous.kind;
                match Self::get_parse_rule(token_kind).infix {
                    Some(infix) => infix(self, can_assign),
                    None => self
                        .parser
                        .borrow_mut()
                        .error("Unexpected infix expression."),
                }
            }

            if can_assign && self.match_token(Kind::Equal) {
                self.parser.borrow_mut().error("Invalid assignment target.");
            }
        } else {
            self.parser
                .borrow_mut()
                .error("expected an expression here.");
        }
    }

    /// consumes the identifier token for the variable name, adds its lexeme
    /// to the chunk’s constant table as a string, and then returns the
    /// constant table index where it was added
    fn parse_variable(&mut self, err_msg: &'static str, is_const: bool) -> usize {
        self.consume(Kind::Identifier, err_msg);
        self.declare_variable(is_const);
        if self.scope_depth > 0 {
            // exit the function if in a local scope.
            return 0;
        }
        let identifier = self.parser.borrow().previous;
        self.identifier_constant(identifier)
    }

    fn declare_variable(&mut self, is_const: bool) {
        let name = self.parser.borrow().previous;
        if self.scope_depth == 0 {
            interner::intern(name.lexeme);
            return;
        }

        // NOTE: search direction here is required to search most
        // recent declarations.
        for local in self.locals.iter().rev() {
            // local.depth < scope depth means the current local is an outer variable
            // search starts from most inward scope out.
            if local.depth != -1 && local.depth < self.scope_depth {
                break;
            }

            if name.lexeme == local.name.lexeme {
                self.parser
                    .borrow_mut()
                    .error("Variable with this name exists in this scope.");
            }
        }
        self.add_local(name, is_const);
    }

    fn add_local(&mut self, token: Token<'src>, immutable: bool) {
        self.locals.push(Local {
            name: token,
            depth: self.scope_depth,
            is_const: immutable,
        });
    }

    fn identifier_constant(&mut self, token: Token) -> usize {
        let name = token.lexeme;
        match interner::get_symbol(name) {
            Some(symbol) => self.current_chunk().add_if_absent(Value::String(symbol)),
            None => {
                // NOTE: although we are reporting an error and halt compilation.
                // we still add the constant to the pool. This is because this function still needs a valid
                // value to return.
                let sym = interner::intern(name);
                let index = self.current_chunk().add_if_absent(Value::String(sym));
                // NOTE: do not report error if it is a native function.
                if crate::std::is_native_call(name) {
                    return index;
                }
                self.parser
                    .borrow_mut()
                    .error_at_current("undeclared variable is being assinged to.");
                index
            }
        }
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
    rules[(Kind::LeftParen as u8) as usize] = ParseRule::new(
        |compiler, _| compiler.grouping(),
        |compiler, _| compiler.call(),
        Precedence::Call,
    );
    rules
};
