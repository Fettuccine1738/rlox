use std::fmt::Display;

#[derive(Debug, Copy, Clone)]
#[repr(u8)] // lets us represent them as bytes as C does.
pub enum OpCode {
    Return = 0, // return from the current function. 
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
            _ => panic!("Invalid opcode {}", b),
        }
    }

}

#[derive(Debug)]
pub struct Chunk {
    pub code: Vec<u8>, // uint8(bits)_t
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new()
        }
    }

    pub fn write_chunk(&mut self, op_code: OpCode) {
        self.code.push(op_code as u8);
    }

    pub fn disassemble(&self, name: &str) {
        println!("====={name}=====");

        for c in &self.code {
            // let as_byte: String = Self::get_bytes(*c);
            let op_code: OpCode = OpCode::from_byte(*c);
            println!("{op_code}");
        }
    }

    // pub fn get_bytes(bits: u8) -> String {
    //     let mut string: String = String::new();
    //     for b in bits.to_be_bytes() {
    //         string.push(b as char);
    //     }
    //     string
    // }
}