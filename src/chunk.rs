use std::{fmt::Display, ops::Neg};

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
}

impl Display for OpCode {
    // add code here
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // Use `self.number` to refer to each positional data point.
        write!(f, "{:08b} {:?}", *self as u8, self)
    }
}

impl OpCode {
    fn from_byte(b: u8) -> Self {
        match b {
            0 => OpCode::Return,
            1 => OpCode::Constant,
            2 => OpCode::ConstantLong,
            _ => panic!("Invalid opcode {}", b),
        }
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
                let idx = self.code[offset + 1];
                println!("  OP_CONSTANT\t{}\t{}", idx, self.constants[idx as usize].0);
                offset + 2
            }
            OpCode::ConstantLong => {
                // 24 bit operand.
                let bytes = &self.code[offset + 1..offset + 4];
                let idx = (bytes[0] as u32) | (bytes[1] as u32) << 8 | (bytes[2] as u32) << 16; // 24 bits
                let constant = self.constants[idx as usize].0;
                println!("  OP_CONSTANT_LONG\t{}\t{constant}", idx);
                offset + 4 // consume op_code_long, byte, byte, byte 
            }, // _ => panic!()
            OpCode::Negate => {
                let constant = -self.constants[offset];
                todo!()
            }
        }
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

    // pub fn get_bytes(bits: u8) -> String {
    //     let mut string: String = String::new();
    //     for b in bits.to_be_bytes() {
    //         string.push(b as char);
    //     }
    //     string
    // }
}
