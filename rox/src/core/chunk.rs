use std::fmt::Display;

use crate::{core::opcode::*, core::value::Value, data_structures::interner};
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq)]
pub struct Line(pub u32);

// CHALLENGE: to generate a minimal instruction set eliminating
// either OP_NEGATE or OP_SUBSTRACT: 4 - 3 * -2
// constant -> op_sub -> constant -> op_mul -> constant 0 -> op_sub -> 2 (removing negation)
// constant -> constant(-ve) -> op_mul -> constant(-2).

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub lines: Vec<Line>,
    // HACK: index_const24 records the size of the bytecode array when the constants pool
    // exceeds 255 (the value at which Constant24 must be used as the operand to store and read constants.)
    // this allows the compiler & vm to compare the instruction ptr with this size
    // if the ip is >= index_const24 we have to read the next 3 bytes to get the correct index.
    pub index_const24: usize,
}

impl Display for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "\nindex_const24 = {} \n {:?}\n {:?}",
            self.index_const24, self.code, self.constants
        )
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    pub const fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
            index_const24: usize::MAX, // Sentinel.
        }
    }

    pub fn save_index(&mut self) {
        self.index_const24 = self.code.len()
    }

    // write allows user to write both opcodes and their operands
    // write chunk only takes in opcodes
    pub fn write(&mut self, byte: u8, line: u32) {
        self.code.push(byte);
        self.lines.push(Line(line));
    }

    pub fn write_chunk(&mut self, op_code: OpCode, line: u32) {
        self.write(op_code as u8, line);
    }

    pub fn disassemble(chunk: &Chunk, name: &str) {
        println!("====={name}=====");
        let mut i = 0usize;

        while i < chunk.code.len() {
            i = Self::disassemble_instruction(chunk, i);
        }
    }

    pub fn disassemble_instruction(chunk: &Chunk, offset: usize) -> usize {
        print!("{:04} ", offset);
        let line = chunk.lines[offset].0;

        if offset > 0 && line == chunk.lines[offset - 1].0 {
            print!("    | ");
        } else {
            print!("{:4} ", line);
        }

        let instruction = chunk.code[offset];
        let op = OpCode::try_from(instruction).expect("instruction not understood");

        // OpConstant -> store bytecode, store index of the value <index is only between 0-255>
        // only 256 possible combinations, problematic if we require more than that.
        // for OpConstant24 -> store bytecode, but index could go up to 24 bits, i.e
        // to get the (operand) index of the value, we may need to look at index1, index2, index3
        match op {
            OpCode::Return => {
                println!(" RETURN");
                offset + 1
            }
            OpCode::Class => chunk.constant_instruction("OP_CLASS", offset),
            OpCode::GetProperty => chunk.constant_instruction("OP_GET_PROPERTY", offset),
            OpCode::SetProperty => chunk.constant_instruction("OP_SET_PROPERTY", offset),
            OpCode::Constant => {
                let index = chunk.read_constant(offset);
                println!("  OP_CONSTANT\t{}\t{}", index, chunk.constants[index]);
                offset + 2
            }
            OpCode::Constant24 => {
                // 24 bit operand.
                let index = chunk.read_long_constant(offset);
                let constant = &chunk.constants[index];
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
            OpCode::True => Self::simple_instruction("OP_TRUE", offset),
            OpCode::False => Self::simple_instruction("OP_FALSE", offset),
            OpCode::NIL => Self::simple_instruction("OP_NIL", offset),
            OpCode::Not => Self::simple_instruction("OP_NOT", offset),
            OpCode::Equal => Self::simple_instruction("OP_EQUAL", offset),
            OpCode::Greater => Self::simple_instruction("OP_GREATER", offset),
            OpCode::Less => Self::simple_instruction("OP_LESS", offset),
            OpCode::Print => Self::simple_instruction("OP_PRINT", offset),
            OpCode::Pop => Self::simple_instruction("OP_POP", offset),
            OpCode::DefineGlobal => chunk.constant_instruction("OP_DEFINE_GLOBAL", offset),
            OpCode::GetGlobal => chunk.constant_instruction("OP_GET_GLOBAL", offset),
            OpCode::SetGlobal => chunk.constant_instruction("OP_SET_GLOBAL", offset),
            OpCode::PopN => chunk.constant_instruction("OP_POP_N", offset),
            OpCode::GetLocal => {
                let slot = chunk.code[offset + 1];
                // -1 because operand to this opcode is the index in its local stack
                // which is offset by 1 because there is always a dummy local.
                println!("OP_GET_LOCAL {:04}", slot);
                offset + 2
            }
            OpCode::SetLocal => chunk.byte_instruction("OP_SET_LOCAL", offset, false),
            OpCode::JumpIfFalse => chunk.jump_instruction("OP_JUMP_IF_FALSE", 1, offset),
            OpCode::Jump => chunk.jump_instruction("OP_JUMP", 1, offset),
            OpCode::Loop => chunk.jump_instruction("OP_LOOP", -1, offset),
            // arity is a byte instruction, because arguments are limited to =255
            OpCode::Call => chunk.byte_instruction("OP_CALL: arity = ", offset, false),
            OpCode::Closure => {
                let (mut off_t, constant) = if offset < chunk.index_const24 {
                    (offset + 2, chunk.code[offset + 1] as usize)
                } else {
                    let bytes = &chunk.code[offset + 1..offset + 4];
                    let index = Self::inverse_resolve(bytes[0], bytes[1], bytes[2]);
                    (offset + 4, index)
                };
                print!("OP_CLOSURE {:04}", constant);
                let function = Value::as_function(&chunk.constants[constant]);
                for _ in 0..function.upvalue_count {
                    // encoding [is_long][idx_1b or idx_3b][is_local]
                    // is_long ? idx_3b : idx_1b (3b = 3bytes. upvalue may point to slot > 255.)
                    let is_long = chunk.code[off_t];
                    off_t += 1;

                    let index: usize = if is_long == 1 {
                        let index = Self::inverse_resolve(
                            chunk.code[off_t],
                            chunk.code[off_t + 1],
                            chunk.code[off_t + 2],
                        );
                        off_t += 3;
                        index
                    } else {
                        let index = chunk.code[off_t] as usize;
                        off_t += 1;
                        index
                    };

                    let is_local = chunk.code[off_t];
                    off_t += 1;

                    println!(
                        "{:04}    |              {} {}",
                        off_t - 2,
                        if is_local == 1 { " local" } else { "upvalue" },
                        index
                    );
                }
                off_t
            }
            OpCode::GetUpValue => chunk.byte_instruction("OP_GET_UPVALUE", offset, false), // operand is code pool
            OpCode::SetUpValue => chunk.byte_instruction("OP_SET_UPVALUE", offset, false), // also here
            OpCode::CloseUpValue => Self::simple_instruction("OP_CLOSE_VALUE", offset),
        }
    }

    fn simple_instruction(name: &str, offset: usize) -> usize {
        println!("   {name}");
        offset + 1
    }

    /// in_const_pool = true, if the operand to this bytecode is an index to the constants pool
    /// For some instructions like closures, the index points into the runtime structure.
    /// causing index out of bounds.
    fn byte_instruction(&self, name: &str, offset: usize, in_const_pool: bool) -> usize {
        // the operand to this opcode is not always in the constants pool, it may be an index
        // in the upvalues or locals list of another function
        let slot = self.code[offset + 1];
        print!("{name} \t");
        if in_const_pool {
            println!("{:04}", self.constants[slot as usize]);
        } else {
            println!("{}", slot);
        }
        offset + 2
    }

    fn jump_instruction(&self, name: &str, sign: i32, offset: usize) -> usize {
        let b8_15 = self.code[offset + 2] as u32;
        let jump = self.code[offset + 1] as u32 | (b8_15 << 8);
        print!("   {name}\t");
        println!("{offset:4} {}", (offset as i32 + 3 + sign * jump as i32));
        offset + 3
    }

    // TODO: fix this, compare with impl in the book!
    fn constant_instruction(&self, name: &str, offset: usize) -> usize {
        print!("   {name}\t");
        let index = self.code[offset + 1]; // index of value is embeded in the bytecode stream.

        #[cfg(any(test, debug_assertions))]
        if let Value::String(id) = self.constants[index as usize] {
            println!("{}", interner::get_string(id).unwrap())
        }

        offset + 2 // consume current bytecode and operand index.
    }

    // reads the corresponding value of the OP_CONSTANT24 operand 24 bits and
    // returns a usize to index into the constants array
    fn read_long_constant(&self, offset: usize) -> usize {
        let bytes = &self.code[offset + 1..offset + 4];
        let idx = (bytes[0] as u32) | (bytes[1] as u32) << 8 | (bytes[2] as u32) << 16; // 24 bits
        idx as usize
    }

    fn read_constant(&self, offset: usize) -> usize {
        let idx = self.code[offset + 1];
        idx as usize
    }

    // constants have an additional operand the index in the constants buffer;
    // 1 or 3 byte is used up depending on the byte_code.
    pub fn write_constant(&mut self, value: Value, line: u32) {
        let idx = self.add_if_absent(value);
        // if the index of stored constant is > 256, we use the OP_CONSTANT_LONG
        if idx < 256 {
            self.code.push(OpCode::Constant as u8);
            self.code.push(idx as u8);
            self.lines.push(Line(line)); // line num for constant bytecode 
            self.lines.push(Line(line)); // line num for constant value
        } else {
            self.code.push(OpCode::Constant24 as u8);
            // resolve byte index.
            let (bits0_7, bits8_15, bits16) = Self::resolve_index(idx);
            self.code.push(bits0_7);
            self.code.push(bits8_15);
            self.code.push(bits16);
            // line num for constant bytecode and 3 line nums for the index.
            self.lines.push(Line(line));
            self.lines.push(Line(line));
            self.lines.push(Line(line));
            self.lines.push(Line(line));
        }
        // !NOTE: remove this assertion when run-length encoding is implemented.
        assert_eq!(self.code.len(), self.lines.len())
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1 // index of the last push
    }

    /// returns the index if this value is already in the constants pool.
    /// otherwise add to the constant pool and return new index;
    /// Optimization to reduce Compiler adding new constant for every use.
    pub fn add_if_absent(&mut self, value: Value) -> usize {
        for (index, constant) in self.constants.iter().enumerate() {
            if constant == &value {
                return index;
            }
        }

        self.add_constant(value)
    }

    pub fn resolve_index(index: usize) -> (u8, u8, u8) {
        let bits = index.to_le();
        let bits0_7 = (bits & 0xFF) as u8;
        let bits8_15 = ((bits >> 8) & 0xFF) as u8;
        let bits16 = ((bits >> 16) & 0xFF) as u8;

        (bits0_7, bits8_15, bits16)
    }

    pub fn inverse_resolve(bits0_7: u8, bits8_15: u8, bits16: u8) -> usize {
        ((bits16 as usize) << 16) | ((bits8_15 as usize) << 8) | (bits0_7 as usize)
    }
}
