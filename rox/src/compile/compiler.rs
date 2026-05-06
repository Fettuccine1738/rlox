use std::cell::RefCell;
use std::rc::Rc;
use std::{mem, vec};

use super::parser::Parser;
use super::token::Kind;
use crate::compile::token::Token;
use crate::core::chunk::Chunk;
use crate::core::opcode::OpCode;
use crate::core::value::Value;
use crate::data_structures::interner::{self};
use crate::runtime::{lang::Function, lang::FunctionType};

pub const FUNCTION_ARG_MAX: u32 = 255;
pub const LONG_UPVALUE_INDEX: u8 = 1; // index of a captured value sitting on slot > 255
pub const SHORT_UPVALUE_INDEX: u8 = 0; // vice versa
// dummy Parse Rule, required in cases where an error occured,
// causing an unexpected TokenKind to be used to indexed the ParseRule table.
// Compiler in some cases doesn't stop.
pub static DEFAULT_ERR_RULE: ParseRule = ParseRule::default();
pub const INIT_KEYWORD: &str = "init"; // for constructors e.g `java` Foo(bar, baz) {}
pub const THIS_KEYWORD: &str = "this";
pub const SUPER_KEYWORD: &str = "super";

#[derive(Debug, Default)]
pub struct Local<'src> {
    name: Token<'src>,
    // record the scope depth of the where the local var was declared.
    // Sentinel -1 means this local is uninitialized.
    depth: i32,
    is_const: bool,
    // true if this local is captured by any later nested function declaration.
    is_captured: bool,
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

#[derive(Debug, Clone, Default)]
pub(crate) struct ClassCompiler<'src> {
    _token: Token<'src>,
    has_super: bool,
}

/// index stores which local slot the upvalue is capturing.
/// HACK: this should be an index since we already allow > 255 constants
/// is_local: is true if it captures a local in its immediate scope.
/// false if capturing an UpValue from the outer scope.
#[derive(Debug)]
pub struct UpValue {
    index: u32,
    is_local: bool,
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
    upvalues: Vec<UpValue>,
    class_stack: Rc<RefCell<Vec<ClassCompiler<'src>>>>,
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
            locals: vec![],
            const_globals: vec![],
            // interior mutabliity, this is so we can return the function after compiling
            // and don't have to worry about `dangling` ptr once compile is finished.
            function: Function::new(),
            function_type: FunctionType::default(),
            enclosing: None,
            upvalues: vec![],
            class_stack: Rc::new(RefCell::new(vec![])),
        };

        // we need this for alignment, the function then looks for params/ args starting from index 1.
        compiler.locals.push(Local {
            name: Token::default(),
            depth: 0,
            is_const: false,
            is_captured: false,
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
        if self.function_type == FunctionType::Init {
            self.emit_opcode_operand(OpCode::GetLocal, 0);
        } else {
            self.emit_opcode(OpCode::NIL);
        }
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

    // emits the opcode and the argument to this opcode
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
        if self.current_chunk().index_const24 == usize::MAX && index > 255 {
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
        #[cfg(feature = "")] // #[cfg(feature="")] // custom features
        // #[cfg(any(test, feature=""))] // analogous to a #ifdef block in C
        let name = self
            .function
            .name
            .as_deref()
            .unwrap_or("Script")
            .to_string();
        #[cfg(feature = "")]
        let status = if self.parser.borrow().had_error {
            "Failed to Compile"
        } else {
            "Compile successful"
        };
        #[cfg(feature = "")]
        let display_string = format!("{}  :  {}", name, status);
        #[cfg(feature = "")]
        Chunk::disassemble(self.current_chunk(), &display_string);

        self.function.free_unused_mem();
        let function = std::mem::take(&mut self.function);
        Rc::new(function)
    }

    /// the current chunk is always the chunk owned by the function currently
    /// being compiled.
    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.function.chunk
    }

    fn declaration(&mut self) {
        if self.match_token(Kind::Class) {
            self.class_declaration();
        } else if self.match_token(Kind::Fun) {
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
            if self.function_type == FunctionType::Init {
                self.parser
                    .borrow_mut()
                    .error("Can't return a value from an initializer")
            }
            self.expression();
            self.consume(Kind::SemiColon, "Expect ';' after return value.");
            self.emit_opcode(OpCode::Return);
        }
    }

    fn class_declaration(&mut self) {
        self.consume(Kind::Identifier, "Expect class name");
        let class_tok = self.parser.borrow().previous;
        // weird thing we have to do so identifier_constant does not throw errors
        let _ = interner::intern(class_tok.lexeme);
        // name_idx is the index of its interned string name in the constants pool
        // this helps the runtime find the class name
        let name_idx = self.identifier_constant(class_tok);
        self.declare_variable(true);
        self.emit_opcode_operand(OpCode::Class, name_idx);
        // the class name (agin index in constant pool) is used to bind
        // instruction to create class object at runtime, takes the constant
        // table index of the class's name as an operand
        self.define_variable(name_idx, true);
        self.class_stack.borrow_mut().push(ClassCompiler {
            _token: class_tok,
            has_super: false,
        });

        // to please the borrow checker
        let mut has_super = false;

        if self.match_token(Kind::Less) {
            self.consume(Kind::Identifier, "Expect superclass name");
            self.variable(false); // emit code to load the superclass 
            let super_tok = self.parser.borrow().previous;
            if class_tok.lexeme == super_tok.lexeme {
                self.parser
                    .borrow_mut()
                    .error("A class can't inherit from itself");
            }
            self.begin_scope();
            self.add_local(Token::synthetic(SUPER_KEYWORD, super_tok.line), true);
            self.define_variable(0, true);

            // BUGBUG: the super keyword is supposed to be defined here but it isn't
            // because  it is not being defined, the compiler thinks we are trying to read its own
            // variable which is a bug itself, class should be declared globally if we are not inside
            // a class.
            self.named_variable(class_tok, false); // load the subclass
            self.emit_opcode(OpCode::Inherit); // connect sub to super
            if let Some(c) = self.class_stack.borrow_mut().last_mut() {
                c.has_super = true;
                has_super = true;
            };
        }
        // loads the class back on top of the stack
        self.named_variable(class_tok, false);

        self.consume(Kind::LeftBrace, "Expect `{` before class body.");
        loop {
            if self.check(Kind::RightBrace) || self.check(Kind::EOF) {
                break;
            }
            self.method();
        }
        self.consume(Kind::RightBrace, "Expect `}` after class body.");
        self.emit_opcode(OpCode::Pop);
        if has_super {
            self.end_scope();
        };

        self.class_stack.borrow_mut().pop();
    }

    fn method(&mut self) {
        self.consume(Kind::Identifier, "Expect method name.");
        let previous = self.parser.borrow().previous;
        let ft = if previous.lexeme.eq(INIT_KEYWORD) {
            FunctionType::Init
        } else {
            FunctionType::Method
        };
        let _ = interner::intern(previous.lexeme);

        let name = self.identifier_constant(previous);
        self.function(ft);
        self.emit_opcode_operand(OpCode::Method, name);
    }

    fn super_(&mut self) {
        if self.class_stack.borrow().is_empty() {
            self.parser
                .borrow_mut()
                .error("can't use `super` outside of a class.");
        } else if let Some(c) = self.class_stack.borrow().last()
            && !c.has_super
        {
            self.parser
                .borrow_mut()
                .error("can't use `super` with no superclass.");
        }

        self.consume(Kind::Dot, "Expect `.` after `super`.");
        self.consume(Kind::Identifier, "Expect superclass method name");
        // method name
        let prev = self.parser.borrow().previous;
        let name_idx = self.identifier_constant(prev);

        // use receiver and superclass to access the method at runtime
        self.named_variable(Token::synthetic(THIS_KEYWORD, prev.line), false);
        if self.match_token(Kind::LeftParen) {
            let arg_count = self.argument_list();
            self.named_variable(Token::synthetic(SUPER_KEYWORD, prev.line), false);
            self.emit_opcode_operand(OpCode::SuperInvoke, name_idx);
            self.emit_byte(arg_count as u8);
        } else {
            self.named_variable(Token::synthetic(SUPER_KEYWORD, prev.line), false);
            self.emit_opcode_operand(OpCode::GetSuper, name_idx);
        }
    }

    fn this(&mut self) {
        if self.class_stack.borrow().is_empty() {
            self.parser
                .borrow_mut()
                .error("Cannot use `this` outside of a class");
        }
        self.variable(false);
    }

    fn dot(&mut self, can_assign: bool) {
        self.consume(Kind::Identifier, "Expect property name after `.`.");
        let previous = self.parser.borrow().previous;
        interner::intern(previous.lexeme);
        let name: usize = self.identifier_constant(previous);

        // TODO: how to support consts fields, if not defined beforehand?
        // to avoid calling a set/get proprty in  a context with high precedence
        // i.e a + b.c = 3 :: compiled as a + (b.c = 3)
        if can_assign && self.match_token(Kind::Equal) {
            self.expression();
            self.emit_opcode_operand(OpCode::SetProperty, name);
        } else if self.match_token(Kind::LeftParen) {
            let arg_count = self.argument_list();
            self.emit_opcode_operand(OpCode::Invoke, name);
            self.emit_byte(arg_count as u8);
        } else {
            self.emit_opcode_operand(OpCode::GetProperty, name);
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
            scope_depth: 0,
            const_globals: Vec::new(),
            function: Function::new(),
            function_type: func_type,
            class_stack: enclosing.class_stack.clone(),
            enclosing: Some(Box::new(enclosing)),
            upvalues: vec![],
        };

        inner.function.name = Some(function_name.to_owned());
        // Slot 0 is reserved for the instance when compiling method calls
        // Functions are however not allowed to use the `this` keyword. so if
        // a function declaration is inside a method it resolves to the enclosing
        // method i.e
        // class ... {
        //  method() {
        //      fun foo() { print this; }
        //      foo();
        //    }
        // }
        let this = if func_type != FunctionType::Function {
            Token {
                kind: Kind::Identifier,
                lexeme: THIS_KEYWORD,
                line: 0,
            }
        } else {
            Token::default()
        };
        inner.locals.push(Local {
            name: this,
            depth: 0,
            is_const: false,
            is_captured: false,
        });

        // consume parameters
        if !inner.check(Kind::RightParen) {
            loop {
                if (inner.function.arity as u32 + 1) > FUNCTION_ARG_MAX {
                    inner
                        .parser
                        .borrow_mut()
                        .error_at_current("Function cannot have more than 255 parameters.");
                }
                inner.function.arity += 1;
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
        // inner.end_scope(); unclear why we do not need to end scope

        // Enclosing compiler holds this closure and emits the bytes and operands
        // to the closure in its own chunk.
        let bytes_to_emit: Vec<(u8, u32)> = inner
            .upvalues
            .iter()
            .map(|u| (if u.is_local { 1 } else { 0 }, u.index))
            .collect();
        let function: Rc<Function> = inner.end_compilation();
        let _inner: Compiler = mem::replace(self, *inner.enclosing.unwrap());

        // value is stored as function but used as closure.
        let index: usize = self
            .current_chunk()
            .add_if_absent(Value::LoxFunction(function));
        // operand to this opcode, is the constant functions index in the constants table.
        // TODO: if bytes_to_emit is empty, we can emit a Function instead of a closure
        self.emit_opcode_operand(OpCode::Closure, index);

        // variable encoding of the byte is now
        // [0 | 1 (is this index > 255)][idx_1b | idx_3b][is_local]
        for (is_local, index) in bytes_to_emit {
            if index > 255 {
                // is_long flag (used in cases where the captured variable) means
                // is the (255+th) variable in the captured local or upvalue.
                self.emit_byte(LONG_UPVALUE_INDEX);
                // emit 3 bytes
                let (bits0_7, bits8_15, bits16) = Chunk::resolve_index(index as usize);
                self.emit_byte(bits0_7);
                self.emit_byte(bits8_15);
                self.emit_byte(bits16);
            } else {
                self.emit_byte(SHORT_UPVALUE_INDEX); // !is_long index into upvalues or locals.
                self.emit_byte(index as u8);
            }
            self.emit_byte(is_local);
        }
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
            Some((index, is_const)) => (OpCode::GetLocal, OpCode::SetLocal, index, is_const),
            None => match self.resolve_upvalue(&name) {
                // NOTE: index refers to index in different tables.
                // for UpValue, it is the index in the upvalues array.
                Some((idx, is_const)) => (OpCode::GetUpValue, OpCode::SetUpValue, idx, is_const),
                _ => {
                    // here it is the index in its chunk constants pool.
                    let idx: usize = self.identifier_constant(name);
                    (
                        OpCode::GetGlobal,
                        OpCode::SetGlobal,
                        idx,
                        self.const_globals.contains(&idx),
                    )
                }
            },
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

    fn resolve_local(&mut self, name: &Token) -> Option<(usize, bool)> {
        for (idx, local) in self.locals.iter().enumerate().rev() {
            if name.lexeme == local.name.lexeme {
                if !local.is_initialized() {
                    self.parser
                        .borrow_mut()
                        .error("Can't read local variable in its own initializer.");
                }
                return Some((idx, local.is_const));
            }
        }
        None
    }

    /// searches for a variable possibly declared in a surrounding function.
    /// if `name.lexeme` is not found amongst its local variables.
    /// returns the index where the variable was found and a bool if its immutable.
    /// By the time the compiler reaches the end of a function declaration,
    /// every variable reference has been resolved as either a local, an upvalue, or a global
    fn resolve_upvalue(&mut self, name: &Token) -> Option<(usize, bool)> {
        // we are currently in the outer most compiler
        let enclosing = self.enclosing.as_mut()?;

        // reucursive call to search all the way back to the outermost compiler.
        let local: Option<(usize, bool)> = enclosing.resolve_local(name);
        // index refers to the index of the slot in its the enclosing locals
        if let Some((index, is_const)) = local {
            // value_index is the index of the captured up value in its own local array.
            enclosing.locals[index].is_captured = true;
            let value_index = self.add_upvalue(index, true);
            return Some((value_index, is_const));
        }

        if let Some((index, is_const)) = enclosing.resolve_upvalue(name) {
            // we know its not local because the enclosing couldn't find in its locals.
            let value_index = self.add_upvalue(index, false);
            Some((value_index, is_const))
        } else {
            None
        }
    }

    /// local = true means a value declared in an immediate outerscope is captured.
    /// false means it captures the UpValue of some captured variable.
    fn add_upvalue(&mut self, index: usize, local: bool) -> usize {
        for (idx, upvalue) in self.upvalues.iter().enumerate() {
            if upvalue.index == index as u32 && upvalue.is_local == local {
                return idx;
            }
        }

        // a function cannot capture more than 255 upvalues.
        // operand to `OpCode::GetUpValue` and `OpCode::SetUpValue` is u8
        // an index into this upvalue array.
        if self.upvalues.len() == FUNCTION_ARG_MAX as usize {
            self.parser
                .borrow_mut()
                .error("Too many closure variables in function.");
        }

        self.upvalues.push(UpValue {
            index: index as u32,
            is_local: local,
        });
        self.function.upvalue_count += 1;
        self.upvalues.len() - 1
    }

    fn define_variable(&mut self, global: usize, is_const: bool) {
        if self.scope_depth > 0 {
            // local scope.
            self.mark_initialized(is_const);
            return;
        }

        if is_const {
            self.const_globals.push(global); // compiler knows index at this slot is immutable
        }
        self.emit_opcode_operand(OpCode::DefineGlobal, global);
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
            self.parser.borrow_mut().advance();
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
        if let Some(jump) = exit_jump {
            self.patch_jump(jump);
            self.emit_opcode(OpCode::Pop);
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
        self.consume(Kind::RightParen, "Expect ')' after condition.");

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
        if offset as u16 > u16::MAX {
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
    fn and(&mut self, _can_assign: bool) {
        let end_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_opcode(OpCode::Pop);
        self.parse_precedence(Precedence::And);
        self.patch_jump(end_jump);
    }

    fn or(&mut self, _can_assign: bool) {
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

        if jump as u16 > u16::MAX {
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

        // iterate through locals and emit code if used by any nested
        // functions.
        while !self.locals.is_empty() && self.locals[self.locals.len() - 1].depth > self.scope_depth
        {
            if self.locals.last().unwrap().is_captured {
                self.emit_opcode(OpCode::CloseUpValue);
            } else {
                self.emit_opcode(OpCode::Pop);
            }
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
        interner::intern(name.lexeme);
        if self.scope_depth == 0 {
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
            is_captured: false,
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
                let err_msg = format!("undeclared variable `{}` is being assigned to.", name);
                self.parser.borrow_mut().error_at_current(&err_msg);
                index
            }
        }
    }

    fn get_parse_rule(kind: Kind) -> &'static ParseRule {
        match RULES.get(kind as usize) {
            Some(rule) => rule,
            None => &DEFAULT_ERR_RULE,
        }
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
    rules[(Kind::This) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.this(), Precedence::None);

    rules[(Kind::Super) as usize] =
        ParseRule::new_prefix(|compiler, _| compiler.super_(), Precedence::None);
    rules[(Kind::Dot) as usize] = ParseRule::new_infix(
        |compiler, can_assign| compiler.dot(can_assign),
        Precedence::Call,
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
