use std::fmt::Display;

// each opcode determines the size of its operands.
// For example, OpCode::return may have no operands.
#[derive(Debug, Copy, Clone)]
#[repr(u8)] // lets us represent them as bytes as C does.
pub enum OpCode {
    Return = 0, // return from the current function.
    Constant = 1,
    Constant24 = 2,
    Negate = 3,
    Add = 4,
    Divide = 5,
    Multiply = 6,
    Subtract = 7,
    NIL = 8,
    True = 9,
    False = 10,
    Not = 11,
    Equal = 12,
    Greater = 13,
    Less = 14,
    Print = 15,
    Pop = 16,
    DefineGlobal = 17,
    GetGlobal = 18,
    SetGlobal = 19,
    GetLocal = 20,
    SetLocal = 21,
    PopN = 22,
    JumpIfFalse = 23,
    Jump = 24,
    Loop = 25,
    Call = 26,
    Closure = 27,
    GetUpValue = 28,
    SetUpValue = 29,
    CloseUpValue = 30,
    // Design choice on why OpCodes for !=, <=, >= are not implemented.
    // the bytecode instructions does not need to follow closely to the user's
    // source code. The VM has total freedom to use whatever instruction set and code sequence
    // as long as they have the right behavior.
    // Semantically: a != b  === !(a == b)
    // a <= b === !(a > b)
    // a >= b === !(a < b). except for floating-point NaN
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
            0 => Ok(Self::Return),
            1 => Ok(Self::Constant),
            2 => Ok(Self::Constant24), // constant opcode whose operand is the next 3 bytes.
            3 => Ok(Self::Negate),
            4 => Ok(Self::Add),
            5 => Ok(Self::Divide),
            6 => Ok(Self::Multiply),
            7 => Ok(Self::Subtract),
            8 => Ok(Self::NIL),
            9 => Ok(Self::True),
            10 => Ok(Self::False),
            11 => Ok(Self::Not),
            12 => Ok(Self::Equal),
            13 => Ok(Self::Greater),
            14 => Ok(Self::Less),
            15 => Ok(Self::Print),
            16 => Ok(Self::Pop),
            17 => Ok(Self::DefineGlobal),
            18 => Ok(Self::GetGlobal),
            19 => Ok(Self::SetGlobal),
            20 => Ok(Self::GetLocal),
            21 => Ok(Self::SetLocal),
            22 => Ok(Self::PopN),
            23 => Ok(Self::JumpIfFalse),
            24 => Ok(Self::Jump),
            25 => Ok(Self::Loop),
            26 => Ok(Self::Call),
            27 => Ok(Self::Closure),
            28 => Ok(Self::GetUpValue),
            29 => Ok(Self::SetUpValue),
            30 => Ok(Self::CloseUpValue),
            _ => Err(()),
        }
    }
}
