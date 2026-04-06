use core::panic;
use std::ops::{Add, Div, Mul, Sub};
use std::rc::Rc;

use string_interner::symbol::SymbolU32;

//------------Virtual-machine
use crate::compile::compiler::Compiler;
use crate::core::chunk::Chunk;
use crate::core::lang::{CallFrame, Closure};
use crate::core::opcode::OpCode;
use crate::core::value::{NativeFn, Value};
use crate::data_structures::interner::{self};
use crate::data_structures::map::HashTable;
use crate::std::math;
use crate::std::time;
use crate::std::{io, strings};

// use crate::lox_errors::VmError;
// use crate::value::HeapAllocatedObj;

pub const DEBUG_TRACE: bool = true;
pub const FRAMES_MAX: usize = 64;
pub const STACK_MAX: usize = 256; // update to FRAMES_MAX * UINT8_COUNT

// PartialEq is derived, to allow assertions on the variants.
#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum InterpretResult {
    Ok,
    CompileError,
    RuntimeError,
}

pub struct VM {
    stack: Vec<Value>,
    globals: HashTable,
    call_frames: Vec<CallFrame>, // for gc ..
                                 // Box::automatically deallocates objects on the heap.
                                 // objects: LinkedList<Value>
}

impl VM {
    pub fn init(&mut self) {
        self.reset_stack();
    }

    pub fn new() -> Self {
        Self {
            // chunk: None, // we don't always start out with valid chunks
            stack: Vec::with_capacity(STACK_MAX),
            globals: HashTable::new(),
            call_frames: Vec::with_capacity(FRAMES_MAX),
        }
    }

    fn reset_stack(&mut self) {
        self.stack.clear();
    }

    pub fn interpret(&mut self, source: String) -> InterpretResult {
        match Compiler::compile(&source) {
            None => InterpretResult::CompileError,
            Some(rc) => {
                self.define_native("io::readNumber".to_owned(), NativeFn(io::read_number));
                self.define_native("io::readLine".to_owned(), NativeFn(io::read_line));
                self.define_native("time::clock".to_owned(), NativeFn(time::clock));
                self.define_native("math::sqrt".to_owned(), NativeFn(math::sqrt));
                self.define_native("math::max".to_owned(), NativeFn(math::max));
                self.define_native("math::pow".to_owned(), NativeFn(math::pow));
                self.define_native("strings::str_cmp".to_owned(), NativeFn(strings::str_cmp));

                // guard against garbage collection.
                self.stack.push(Value::LoxFunction(rc.clone()));
                self.stack.pop();
                //---
                let closure = Rc::new(Closure::new(rc));
                self.stack.push(Value::LoxClosure(closure.clone()));
                self.call(closure, 0);
                self.run()
            }
        }
    }

    pub fn push_value(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    fn peek(&mut self, distance: usize) -> Value {
        self.stack[self.stack.len() - 1 - distance].clone()
    }

    fn get_current_frame_mut(&mut self) -> &mut CallFrame {
        self.call_frames.last_mut().unwrap()
    }

    fn get_current_frame(&self) -> &CallFrame {
        self.call_frames.last().unwrap()
    }

    fn run(&mut self) -> InterpretResult {
        println!("{:?}", self.stack);
        loop {
            #[cfg(debug_assertions)]
            if DEBUG_TRACE {
                let start = self.get_current_frame().ip;
                Chunk::disassemble_instruction(&self.get_current_frame().closure.function.chunk, start);
            }

            // short lived borrows because borrow checker complains about
            // when explicitly borrowed let s = last();
            // self.get_current_frame_mut().ip += 1; // point to the next instruction to read.
            let instruction: OpCode = OpCode::try_from(self.read_byte()).expect("");
            // println!("debuging {}", instruction);

            match instruction {
                OpCode::Return => {
                    if let Some(result) = self.stack.pop() {
                        // pop call frame, this is fine since call frames only track what part of the code
                        // we are in.
                        self.call_frames.pop();
                        // empty means we have finished exectuing the top level code.
                        if self.call_frames.is_empty() {
                            self.stack.pop(); // pop main script function on the stack.
                            println!("{}", result);
                            return InterpretResult::Ok;
                        }
                        let offset = self.get_current_frame().slots;
                        self.stack.truncate(offset);
                        self.stack.push(result);
                    } else {
                        return InterpretResult::RuntimeError;
                    }
                }
                OpCode::Constant => {
                    let constant: Value = self.read_constant();
                    println!("{}", constant);
                    self.stack.push(constant); // self.push_value(constant)
                }
                OpCode::Constant24 => {
                    let constant: Value = self.read_constant();
                    println!("{}", constant);
                    self.stack.push(constant);
                }
                OpCode::Negate => {
                    if !Value::is_number(&self.stack[self.stack.len() - 1]) {
                        self.runtime_error("Operand must be a number.");
                        return InterpretResult::RuntimeError;
                    } else {
                        let num_value = self.stack.pop().unwrap();
                        self.stack.push((-num_value).unwrap());
                    }
                }
                OpCode::Add
                | OpCode::Divide
                | OpCode::Multiply
                | OpCode::Subtract
                | OpCode::Greater
                | OpCode::Less => {
                    let rhs = self.stack.pop().unwrap();
                    let lhs = self.stack.pop().unwrap();
                    match Self::binary_op(lhs, rhs, instruction) {
                        Some(result) => self.stack.push(result),
                        None => return InterpretResult::RuntimeError,
                    }
                }
                OpCode::NIL => {
                    self.stack.push(Value::Nil);
                }
                OpCode::True => {
                    self.stack.push(Value::Boolean(true));
                }
                OpCode::False => {
                    self.stack.push(Value::Boolean(false));
                }
                OpCode::Not => {
                    let value: bool = if let Some(v) = self.stack.pop() {
                        v.is_falsey()
                    } else {
                        return InterpretResult::RuntimeError;
                    };
                    self.stack.push(Value::Boolean(value));
                }
                OpCode::Equal => {
                    let p = self.stack.pop();
                    let q = self.stack.pop();
                    let eq = match (p, q) {
                        (Some(a), Some(b)) => Value::values_equal(a, b),
                        _ => panic!("expected two operands to binary op == "),
                    };
                    self.stack.push(Value::Boolean(eq));
                }
                OpCode::Print => {
                    let value = self.stack.pop().unwrap();
                    println!("PRINT = {}", value);
                }
                OpCode::Pop => {
                    // used for expression stmts to evaluate an expression and
                    // discard the result.
                    let _ = self.stack.pop();
                }
                OpCode::PopN => {
                    // simple optimization to pop all elements at once.
                    let n: u8 = self.read_byte();
                    self.stack.truncate(n as usize);
                }
                OpCode::DefineGlobal | OpCode::ConstGlobal => {
                    // used to strore the global Variable and Value pairs.
                    let name = self.read_string().unwrap();
                    // NOTE: Value is not popped directly off the stack.
                    // This is to ensure that the VM can still find the value after/during garbage collection.b
                    let value = self.peek(0);
                    self.globals.insert(name, value);
                    self.stack.pop(); // value is associated with this variable and not needed on the stack. access with variable name
                }
                OpCode::GetGlobal => {
                    let name = self.read_string().unwrap();
                    // here is the actual value associated with this variable name.
                    let value: Value = match self.globals.get(name) {
                        Some(value) => value,
                        None => return InterpretResult::RuntimeError,
                    };
                    self.stack.push(value);
                }
                OpCode::SetGlobal => {
                    let symbol: SymbolU32 = self.read_string().unwrap();
                    let current: Value = self.peek(0);

                    // Throw RuntimeError if assignment to an undeclared global variable.
                    // insert returns true if a no previous value was declared with this variable name.
                    // false otherwise.
                    if self.globals.insert(symbol, current) {
                        self.globals.delete(symbol);
                        Self::runtime_error(
                            self,
                            format!(
                                "Undefinded Variable  '{}'.",
                                interner::get_string(symbol).unwrap()
                            )
                            .as_str(),
                        );
                        return InterpretResult::RuntimeError;
                    }
                }
                OpCode::GetLocal => {
                    // reads the current frame slots array
                    // which meant it accessed the given numbered slot relative to the beginning of that frame.
                    let offset = self.read_byte();
                    // verify correctness
                    let base: usize = self.get_current_frame_mut().slots;
                    let value = self.stack[base + (offset as usize)].clone();
                    self.push_value(value);
                }
                OpCode::SetLocal => {
                    let slot = self.read_byte();
                    let base: usize = self.get_current_frame_mut().slots;
                    self.stack[base + slot as usize] = self.peek(0);
                }
                OpCode::JumpIfFalse => {
                    let offset = self.read_short();
                    if self.peek(0).is_falsey() {
                        self.get_current_frame_mut().ip += offset as usize;
                    }
                }
                OpCode::Jump => {
                    let offset = self.read_short();
                    self.get_current_frame_mut().ip += offset as usize;
                }
                OpCode::Loop => {
                    let offset = self.read_short();
                    self.get_current_frame_mut().ip -= offset as usize;
                }
                OpCode::Call => {
                    let arity = self.read_byte();
                    let function = self.peek(arity as usize);
                    if !self.call_value(function, arity) {
                        return InterpretResult::RuntimeError;
                    }
                    // we are supposed to  update the old frame
                    // frame = &vm.frames[vm.frame_count - 1];
                }
                OpCode::Closure => {
                    let value = self.read_constant();
                    let function = Value::as_function(&value);
                    let closure = Closure::clone(&function);
                    self.stack.push(Value::LoxClosure(Rc::new(closure)));
                }
                _ => todo!(),
            }
        }
    }

    fn call_value(&mut self, callee: Value, arity: u8) -> bool {
        if Value::is_object(&callee) {
            return match &callee {
                // Value::LoxFunction(_) => { represented now as closure.
                //     self.call(Value::as_function(&callee), arity)
                // }
                Value::NativeFunction(func) => {
                    let arg_start = self.stack.len() - arity as usize;
                    let args: &[Value] = &self.stack[arg_start..]; // send only the args the functions need
                    match (func.0)(arity as usize, args) {
                        Ok(result) => {
                            // let trunc = self.stack.len() - (arg_start + 1);
                            self.stack.truncate(arg_start - 1); // remove function and its arguments.
                            self.push_value(result);
                            true;
                        }
                        Err(e) => self.runtime_error(&e.to_string()),
                    }
                   false
                }
                Value::LoxClosure(_) =>   self.call(Value::as_closure(&callee), arity),
                _ => false,
            };
        }
        self.runtime_error("Can only call functions and classes.");
        false
    }

    // takes name of function and the Funtion ptr
    fn define_native(&mut self, name: String, function: NativeFn) {
        let symbol = interner::intern(&name);
        // done because Garbage collection can be triggered anywhere.
        self.push_value(Value::String(symbol));
        self.push_value(Value::NativeFunction(function));

        self.globals.insert(symbol, self.stack[1].clone());
        self.pop();
        self.pop();
    }

    fn call(&mut self, clojure: Rc<Closure>, arity: u8) -> bool {
        if arity != clojure.function.arity {
            let err_msg: String =
                format!("Expected {} arguments but got {}", clojure.function.arity, arity);
            Self::runtime_error(self, &err_msg);
            return false;
        }

        if self.call_frames.len() == FRAMES_MAX {
            Self::runtime_error(self, "Stack overflow");
            return false;
        }
        // [ fn ] [ arg0 ] [ arg1 ] [ arg2 ]  <-- stackTop
        // ^      | -------args to fn ------
        // slots points here (slot 0 = the function being called)
        self.call_frames.push(CallFrame {
            closure: clojure.clone(),
            ip: 0,
            slots: self.stack.len() - arity as usize - 1,
        });
        true
    }

    fn runtime_error(&mut self, msg: &str) {
        eprintln!("{}", msg);
        for i in (0..self.call_frames.len()).rev() {
            let frame: &CallFrame = &self.call_frames[i];
            // - 1 because ip points to the next instruction to be executed
            // but the failed instruction was the previous one.
            let instruction: usize = frame.ip - 1;
            let line = frame.closure.function.chunk.lines[instruction];
            eprint!("[line {}] in ", line.0);
            match &frame.closure.function.name {
                Some(name) => eprintln!("{}", name),
                None => eprintln!("Script"),
            }
            eprint!("[line {}] in ", line.0);
        }
        self.reset_stack();
    }

    // is_long : when opcode is OP_CONSTANT_LONG: Operand is 24bits.
    fn read_constant(&mut self) -> Value {
        let is_long = self.call_frames.last().unwrap().read_long();
        let index = if is_long {
            let b1 = self.read_byte() as u32;
            let b2 = self.read_byte() as u32;
            let b3 = self.read_byte() as u32;

            b1 | (b2 << 8) | (b3 << 16)
        } else {
            self.read_byte() as u32
        };

        self.call_frames
            .last()
            .unwrap()
            .closure
            .function
            .chunk
            .constants
            .get(index as usize)
            .expect("Invalid constant index.")
            .clone()
    }

    // reads the 16 bit operand for jump opCodes
    // retrurns a u16
    fn read_short(&mut self) -> u16 {
        // le order
        let b0_7 = self.read_byte() as u16;
        let b8_15 = self.read_byte() as u16;
        b0_7 | b8_15 << 8
    }

    fn read_byte(&mut self) -> u8 {
        let call_frame = self.call_frames.last_mut().unwrap();
        let byte_code: &u8 = call_frame.closure.function.chunk.code.get(call_frame.ip).unwrap();
        call_frame.ip += 1; // point to next byte_code.
        *byte_code
    }

    fn binary_op(lhs: Value, rhs: Value, opcode: OpCode) -> Option<Value> {
        match opcode {
            OpCode::Add => lhs.add(rhs),
            OpCode::Divide => lhs.div(rhs),
            OpCode::Multiply => lhs.mul(rhs),
            OpCode::Subtract => lhs.sub(rhs),
            OpCode::Greater => Value::greater_than(&lhs, &rhs),
            OpCode::Less => Value::less_than(&lhs, &rhs),
            _ => None,
        }
    }

    fn read_string(&mut self) -> Option<SymbolU32> {
        // Because we have 2 constant-indexing Operands OpConstant and OpConstant24
        // We need to resolve what operand was used to store this constant.
        // so we know to read either the next byte or next 3 bytes.
        match self.read_constant() {
            Value::String(symbol) => Some(symbol), // interner::get_string(symbol),
            _ => None,
        }
    }
}
