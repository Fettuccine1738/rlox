use core::panic;
use std::ops::{Add, Div, Mul, Sub};

//------------Virtual-machine
use crate::chunk::Chunk;
use crate::chunk::OpCode;
use crate::compiler::Compiler;
use crate::data_structures::HashTable;
use crate::value::Value;
use crate::data_structures::interner::{self};

// use crate::lox_errors::VmError;
// use crate::value::HeapAllocatedObj;

pub const DEBUG_TRACE: bool = true;
pub const STACK_MAX: usize = 256;


// PartialEq is derived, to allow assertions on the variants.
#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum InterpretResult {
    Ok,
    CompileError,
    RuntimeError,
}

pub struct VM {
    // chunk: Option<Chunk>,
    ip: usize, // instruction pointer: book uses uint8_t*
    stack: Vec<Value>,
    globals: HashTable,
    // for gc ..
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
            ip: 0usize,
            stack: Vec::with_capacity(STACK_MAX),
            globals: HashTable::new()
        }
    }

    fn reset_stack(&mut self) {
        self.stack.clear();
    }

    pub fn compile(&mut self, source: String) -> InterpretResult {
        let mut chunk: Chunk = Chunk::new();
        if !Compiler::compile(&source, &mut chunk) {
            InterpretResult::CompileError
        } else {
            InterpretResult::Ok
        }
    }

    pub fn interpret(&mut self, source: String) -> InterpretResult {
        let mut chunk: Chunk = Chunk::new();
        if !Compiler::compile(&source, &mut chunk) {
            drop(chunk);
            return InterpretResult::CompileError;
        }
        self.run(&chunk)
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

    fn run(&mut self, chunk: &Chunk) -> InterpretResult {
        loop {
            #[cfg(debug_assertions)]
            if DEBUG_TRACE {
                println!("{:?}", self.stack);
                Chunk::disassemble_instruction(chunk, self.ip);
            }

            let instruction: OpCode = OpCode::try_from(self.read_byte(chunk)).expect("");
            println!("debuging {}", instruction);

            match instruction {
                // OpCode::Return => {
                //     if let Some(v) = self.stack.pop() {
                //         println!("{}", v);
                //     }
                //     return InterpretResult::Ok;
                // }
                OpCode::Constant => {
                    let constant: Value = self.read_constant(chunk, false);
                    println!("{:?}", constant);
                    self.stack.push(constant); // self.push_value(constant)
                }
                OpCode::Constant24 => {
                    let constant: Value = self.read_constant(chunk, true);
                    println!("{:?}", constant);
                    self.stack.push(constant);
                }
                OpCode::Negate => {
                    if !Value::is_number(&self.stack[self.stack.len() - 1]) {
                        Self::runtime_error(self, chunk, "Operand must be a number.");
                        return InterpretResult::RuntimeError;
                    } else {
                        let num_value = self.stack.pop().unwrap();
                        self.stack.push((-num_value).unwrap());
                    }
                }
                OpCode::Add | OpCode::Divide | OpCode::Multiply | OpCode::Subtract => {
                    let rhs = self.stack.pop().unwrap();
                    let lhs = self.stack.pop().unwrap();
                    let result =
                        Self::binary_op(lhs, rhs, instruction).ok_or(InterpretResult::RuntimeError);
                    self.stack.push(result.unwrap());
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
                    let value = self.stack.pop();
                    println!("{:?}", value);
                }
                OpCode::Pop => {
                    // used for expression stmts to evaluate an expression and
                    // discard the result.
                    let _ = self.stack.pop();
                }
                OpCode::DefinedGlobal => { 
                    // used to strore the global Variable and Value pairs.
                    let name: String = self.read_string(chunk).unwrap();
                    // NOTE: Value is not popped directly off the stack.
                    // This is to ensure that the VM can still find the value after/during garbage collection.b
                    let value = self.peek(0);
                    self.globals.insert(name, value);
                    self.stack.pop(); // discard
                }
                OpCode::GetGlobal => {
                    let name: String = self.read_string(chunk).unwrap();
                    // here is the actual value associated with this variable name.
                    let value: Value = match self.globals.get(&name) {
                        Some(value) => value,
                        None => return InterpretResult::RuntimeError
                    };
                    self.stack.push(value);
                }
                _ => todo!(),
            }
        }
    }

    fn runtime_error(vm: &mut VM, chunk: &Chunk, msg: &'static str) {
        eprintln!("{}", msg);
        let instruction: usize = vm.ip - 1;
        let line = chunk.lines[instruction];
        eprintln!("[line {}] in script", line.0);
        vm.reset_stack();
    }

    // is_long : when opcode is OP_CONSTANT_LONG: Operand is 24bits.
    fn read_constant(&mut self, chunk: &Chunk, is_long: bool) -> Value {
        let index = if is_long {
            let b1 = self.read_byte(chunk) as u32;
            let b2 = self.read_byte(chunk) as u32;
            let b3 = self.read_byte(chunk) as u32;

            b1 | (b2 << 8) | (b3 << 16)
        } else {
            self.read_byte(chunk) as u32
        };

        chunk
            .constants
            .get(index as usize)
            .expect("Invalid constant index.")
            .clone()
    }

    fn read_byte(&mut self, chunk: &Chunk) -> u8 {
        let byte_code: u8 = *chunk.code.get(self.ip).unwrap();
        self.ip += 1; // point to next byte_code.
        byte_code
    }

    fn binary_op(lhs: Value, rhs: Value, opcode: OpCode) -> Option<Value> {
        match opcode {
            OpCode::Add => lhs.add(rhs),
            OpCode::Divide => lhs.div(rhs),
            OpCode::Multiply => lhs.mul(rhs),
            OpCode::Subtract => lhs.sub(rhs),
            _ => None,
        }
    }

    fn read_string(&mut self, chunk: &Chunk) -> Option<String> {
        // HACK + TODO: Because we have 2 constant-indexing Operands OpConstant and OpConstant24    
        // We need to resolve what operand was used to store this constant.
        // so we know to read either the next byte or next 3 bytes.
        // This is a pending workaround until a solution is found.
        match self.read_constant(chunk, false) {
            Value::String(symbol) => interner::get_string(symbol),
            _ => None,
        }
    }
}
