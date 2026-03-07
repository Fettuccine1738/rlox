use std::ops::{Add, Div, Mul, Sub};

//------------Virtual-machine
use crate::chunk::Chunk;
use crate::chunk::OpCode;
use crate::compiler::Compiler;
use crate::lox_errors::VmError;
use crate::value::Value;

pub const DEBUG_TRACE: bool = true;
pub const STACK_MAX: usize = 256;

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
        }
    }

    fn reset_stack(&mut self) {
        self.stack.clear();
    }

    pub fn compile(&mut self, source: String) -> InterpretResult {
        let mut chunk: Chunk = Chunk::new();
        if !Compiler::compile(&source, &mut chunk) {
            InterpretResult::CompileError
        } else { InterpretResult::Ok }
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
        self.stack[self.stack.len() - 1 - distance]
    }

    fn run(&mut self, chunk: &Chunk) -> InterpretResult {
        loop {
            #[cfg(debug_assertions)]
            if DEBUG_TRACE {
                println!("{:?}", self.stack);
                Chunk::disassemble_instruction(chunk, self.ip);
            }

            let instruction: OpCode = OpCode::try_from(self.read_byte(chunk)).expect("");

            match instruction {
                OpCode::Return => {
                    if let Some(v) = self.stack.pop() {
                        println!("{}", v);
                    }
                    return InterpretResult::Ok;
                }
                OpCode::Constant => {
                    let constant: Value = self.read_constant(chunk, false);
                    self.stack.push(constant); // self.push_value(constant)
                    println!("{:?}", constant);
                }
                OpCode::Constant24 => {
                    let constant: Value = self.read_constant(chunk, true);
                    self.stack.push(constant);
                    println!("{:?}", constant);
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
                    let result = Self::binary_op(lhs, rhs, instruction);
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
                    let value: bool = Self::is_falsey(self.stack.pop().unwrap());
                    self.stack.push(Value::Boolean(value));
                }
                OpCode::Equal => {
                    let a = self.stack.pop();
                    let b = self.stack.pop();
                    let c = Self::values_equal(a.unwrap(), b.unwrap());
                    self.stack.push(Value::Boolean(c));
                }
                _ => todo!(),
            }
        }
    }

    fn values_equal(a: Value, b: Value) -> bool {
        match (a, b) {
            (Value::Boolean(av), Value::Boolean(bv)) => av == bv,
            (Value::Nil, Value::Nil) => true,
            (Value::Number(av), Value::Number(bv)) => av == bv,
            _ => false,
        }
    }

    // falsiness handles how other types are negated('not'ed)
    // e.g !nil, !"string"
    fn is_falsey(value: Value) -> bool {
        Value::is_nil(&value) || (Value::is_bool(&value) && !Value::as_bool(&value))
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

        *chunk
            .constants
            .get(index as usize)
            .expect("Invalid constant index.")
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
            _ => panic!(),
        }
    }
}
