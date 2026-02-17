use core::error;
use std::ops::Add;
use std::ops::Div;
use std::ops::Mul;
use std::ops::Sub;

//------------Virtual-machine
use crate::chunk::Chunk;
use crate::chunk::OpCode;
use crate::chunk::Value;
use crate::lox_errors::VmError;

pub const DEBUG_TRACE: bool = true;
pub const STACK_MAX: usize = 256;

#[repr(u8)]
pub enum InterpretResult {
    Ok,
    CompileError,
    RuntimeError,
    Undefined,
}

pub struct VM {
    chunk: Chunk,
    ip: usize, // instruction pointer: book uses uint8_t*
    stack: Vec<Value>,
}

impl VM {
    pub fn init(&mut self) {
        self.reset_stack();
    }

    pub fn new(chunk_: Chunk) -> Self {
        Self {
            chunk: chunk_,
            ip: 0usize,
            stack: Vec::with_capacity(STACK_MAX),
        }
    }

    fn reset_stack(&mut self) {
        self.stack.clear();
    }

    pub fn interpret(&mut self, source: String) -> InterpretResult {
        crate::compiler::compile(source);
        InterpretResult::Ok
    }

    pub fn push_value(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    fn run(&mut self) -> InterpretResult {
        loop {
            if DEBUG_TRACE {
                println!("{:?}", self.stack);
                self.chunk.disassemble_instruction(self.ip);
            }
            let instruction: OpCode = OpCode::try_from(self.read_byte()).expect("");
            match instruction {
                OpCode::Return => {
                    if let Some(v) = self.stack.pop() {
                        println!("{}", v);
                    }
                    return InterpretResult::Ok;
                }
                OpCode::Constant => {
                    let constant: Value = self.read_constant(false);
                    self.stack.push(constant); // self.push_value(constant)
                    println!("{:?}", constant);
                    return InterpretResult::Undefined;
                    // break;
                }
                OpCode::ConstantLong => {
                    let constant: Value = self.read_constant(true);
                    self.stack.push(constant);
                    println!("{:?}", constant);
                    return InterpretResult::Undefined;
                    // break;
                }
                OpCode::Negate => {
                    if let Some(constant) = self.stack.pop() {
                        self.stack.push(-constant);
                    }
                }
                OpCode::Add | OpCode::Divide | OpCode::Multiply | OpCode::Subtract => {
                    let rhs = self.stack.pop().unwrap();
                    let lhs = self.stack.pop().unwrap();
                    let result = Self::binary_op(lhs, rhs, instruction);
                    self.stack.push(result);
                    return InterpretResult::Undefined;
                }
                _ => todo!(),
            }
        }
    }

    fn read_constant(&mut self, is_long: bool) -> Value {
        if is_long {
            // let index = self.chunk.read_
            let bytes = &self.chunk.code[self.ip..self.ip + 3];
            let index = (bytes[0] as u32) | (bytes[1] as u32) << 8 | (bytes[2] as u32) << 16;
            let value = self.chunk.constants[index as usize];
            self.ip += 3; // point to next byte code to consume.
            return value;
        } else {
            let index = self.read_byte() as usize;
            // self.ip += 1;
            *self.chunk.constants.get(index).unwrap()
        }
    }

    fn read_byte(&mut self) -> u8 {
        let byte_code: u8 = *self.chunk.code.get(self.ip).unwrap();
        self.ip += 1; // point to next byte_code.
        byte_code
    }

    fn binary_op(lhs: Value, rhs: Value, opcode: OpCode) -> Value {
        match opcode {
            OpCode::Add => lhs.add(rhs),
            OpCode::Divide => lhs.div(rhs),
            OpCode::Multiply => lhs.mul(rhs),
            OpCode::Subtract => lhs.sub(rhs),
            _ => panic!(),
        }
    }
}
