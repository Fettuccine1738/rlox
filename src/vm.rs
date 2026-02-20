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

    pub fn interpret(&mut self, source: String) -> InterpretResult {
        let mut chunk: Chunk = Chunk::new();
        if !crate::compiler::compile(source, &mut chunk) {
            drop(chunk);
            return InterpretResult::CompileError;
        }

        // self.ip  = chunk.code;
        // self.chunk = Some(chunk);
        self.run(&chunk)
    }

    pub fn push_value(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    fn run(&mut self, chunk: &Chunk) -> InterpretResult {
        loop {
            if DEBUG_TRACE {
                println!("{:?}", self.stack);
                chunk.disassemble_instruction(self.ip);
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
                    return InterpretResult::Undefined;
                    // break;
                }
                OpCode::ConstantLong => {
                    let constant: Value = self.read_constant(chunk, true);
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

    // is_long : when opcode is OP_CONSTANT_LONG: Operand is 24bits.
    fn read_constant(&mut self, chunk: &Chunk, is_long: bool) -> Value {
        let index = if is_long {
            // let index = self.chunk.read_
            // let bytes = &chunk.code[self.ip..self.ip + 3];
            // let index = (bytes[0] as u32) | (bytes[1] as u32) << 8 | (bytes[2] as u32) << 16;
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
