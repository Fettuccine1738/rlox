use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Neg, Sub},
};

// each opcode determines how many operand bytes it has and what they mean.
// For example, return may have no operands.
// Each new opcode should specify what its operands look like.
#[derive(Debug, Copy, Clone)]
#[repr(u8)] // lets us represent them as bytes as C does.
pub enum OpCode {
    Return = 0, // return from the current function.
    Constant = 1,
    ConstantLong = 2,
    Negate = 3,
    Add = 4,
    Divide = 5,
    Multiply = 6,
    Subtract = 7,
}

impl Display for OpCode {
    // add code here
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // Use `self.number` to refer to each positional data point.
        write!(f, "{:08b} {:?}", *self as u8, self)
    }
}

impl TryFrom<u8> for OpCode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(OpCode::Return),
            1 => Ok(OpCode::Constant),
            2 => Ok(OpCode::ConstantLong),
            3 => Ok(OpCode::Negate),
            4 => Ok(OpCode::Add),
            5 => Ok(OpCode::Divide),
            6 => Ok(OpCode::Multiply),
            7 => Ok(OpCode::Subtract),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Value(pub f64);

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Neg for Value {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Add for Value {
    type Output = Self;
    fn add(self, other: Self) -> Self::Output {
        Self(self.0 + other.0)
    }
}

impl Div for Value {
    type Output = Self;
    fn div(self, other: Self) -> Self::Output {
        Self(self.0 / other.0)
    }
}

impl Mul for Value {
    type Output = Self;
    fn mul(self, other: Self) -> Self::Output {
        Self(self.0 * other.0)
    }
}

impl Sub for Value {
    type Output = Self;
    fn sub(self, other: Self) -> Self::Output {
        Self(self.0 - other.0)
    }
}

// CHALLENGE: to generate a minimal instruction set eliminating
// either OP_NEGATE or OP_SUBSTRACT: 4 - 3 * -2
// constant -> op_sub -> constant -> op_mul -> constant 0 -> op_sub -> 2 (removing negation)
// constant -> constant(-ve) -> op_mul -> constant(-2).

#[derive(Debug)]
pub struct Chunk {
    pub code: Vec<u8>, // uint8(bits)_t
    pub constants: Vec<Value>,
    pub lines: Vec<Line>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
        }
    }

    // write allows user to write both opcodes and their operands
    // write chunk only takes in opcodes and would fail if operands are passed to it.
    pub fn write(&mut self, byte: u8, line: u32) {
        self.code.push(byte);
        self.lines.push(Line(line));
    }

    pub fn write_chunk(&mut self, op_code: OpCode, line: u32) {
        self.code.push(op_code as u8);
        self.lines.push(Line(line));
    }

    pub fn disassemble(&self, name: &str) {
        println!("====={name}=====");
        let mut i = 0usize;

        while i < self.code.len() {
            i = self.disassemble_instruction(i);
        }
    }

    pub fn disassemble_instruction(&self, offset: usize) -> usize {
        print!("{:04} ", offset);
        let line = self.lines[offset].0;

        if offset > 0 && line == self.lines[offset - 1].0 {
            print!("    | ");
        } else {
            print!("{:4} ", line);
        }

        let instruction = self.code[offset];
        let op = OpCode::try_from(instruction).expect("");

        // OpConstant -> store bytecode, store index of the value <index is only between 0-255>
        // only 256 possible combinations, problematic if we require more than that.
        // for OpConstantLong -> store bytecode, but index could go up to 24 bits, i.e
        // to get the (operand) index of the value, we may need to look at index1, index2, index3
        match op {
            OpCode::Return => {
                println!(" RETURN");
                offset + 1
            }
            OpCode::Constant => {
                let index = self.read_constant(offset);
                println!("  OP_CONSTANT\t{}\t{}", index, self.constants[index].0);
                offset + 2
            }
            OpCode::ConstantLong => {
                // 24 bit operand.
                let index = self.read_long_contant(offset);
                let constant = self.constants[index].0;
                println!("  OP_CONSTANT_LONG\t{}\t{constant}", index);
                offset + 4 // consume op_code_long, byte, byte, byte 
            } // _ => panic!()
            OpCode::Negate | OpCode::Add | OpCode::Divide | OpCode::Multiply | OpCode::Subtract => {
                // It is impossible to know what value is being negated at disassembly time.
                // e.g OP_CONSTANT 1, OP_CONSTANT_LONG 2, OP_ADD, OP_NEGATE
                // how do we know what expression the sign is being applied onto.
                println!("  OP_{:?}", op);
                offset + 1
            }
            _ => todo!(),
        }
    }

    // reads the corresponding value of the OP_CONSTANT_LONG operand 24 bits and
    // returns its a usize to index into the constants array
    fn read_long_contant(&self, offset: usize) -> usize {
        let bytes = &self.code[offset + 1..offset + 4];
        let idx = (bytes[0] as u32) | (bytes[1] as u32) << 8 | (bytes[2] as u32) << 16; // 24 bits
        return idx as usize;
    }

    fn read_constant(&self, offset: usize) -> usize {
        let idx = self.code[offset + 1];
        idx as usize
    }

    // constants have an additional operand the index in the constants buffer;
    // 1 or 3 byte is used up depending on the byte_code.
    pub fn write_constant(&mut self, value: f64, line: u32) {
        let idx = self.add_constant(value);
        // if the index of stored constant is > 256, we use the OP_CONSTANT_LONG
        if idx < 256 {
            self.code.push(OpCode::Constant as u8);
            self.code.push(idx as u8);
            self.lines.push(Line(line)); // line num for constant bytecode 
            self.lines.push(Line(line)); // line num for constant value
        } else {
            self.code.push(OpCode::ConstantLong as u8);
            // resolve byte index.
            let bits = idx.to_le();
            self.code.push((bits & 0xFF) as u8);
            self.code.push(((bits >> 8) & 0xFF) as u8);
            self.code.push(((bits >> 16) & 0xFF) as u8);
            // line num for constant bytecode and 3 line nums for the index.
            self.lines.push(Line(line));
            self.lines.push(Line(line));
            self.lines.push(Line(line));
            self.lines.push(Line(line));
        }
        // !NOTE: remove this assertion when run-length encoding is implemented.
        assert_eq!(self.code.len(), self.lines.len())
    }

    pub fn add_constant(&mut self, value: f64) -> usize {
        self.constants.push(Value(value));
        self.constants.len() - 1 // index of the last push
    }
}
