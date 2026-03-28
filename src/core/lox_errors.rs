use std::fmt;

#[derive(Debug)]
pub enum VmError {
    StackOverflow,
    InvalidOpcode(u8),
    Runtime(String),
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::StackOverflow => write!(f, "Stackunderflow"),
            VmError::InvalidOpcode(op) => write!(f, "Invalid opcode: {}", op),
            VmError::Runtime(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
}

impl std::error::Error for VmError {}

impl From<std::io::Error> for VmError {
    fn from(err: std::io::Error) -> Self {
        VmError::Runtime(err.to_string())
    }
}
