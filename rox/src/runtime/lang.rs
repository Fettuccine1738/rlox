use crate::core::chunk::Chunk;
use crate::core::value::ObjId;
use crate::runtime::heap::{GcValue, Heap};
use std::fmt::Display;

/// NOTE: move to object.rs once complexity increases.
#[derive(Debug, Clone)]
pub struct Function {
    // like Java limit a function's parameter count to < 255
    // < 255 because methods, take self implicitly as an argument
    pub arity: u8,
    pub chunk: Chunk,
    pub name: Option<String>,
    pub upvalue_count: usize,
}

impl PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        self.arity == other.arity && self.name == other.name
    }
}

impl PartialOrd for Function {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.arity.partial_cmp(&other.arity) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.name.partial_cmp(&other.name)
    }
}

/// A CallFrame represents a single ongoing function call. The slots field
/// points into the VM's value stack at the first slot that this function
/// can use.
#[derive(Debug, Clone, Copy)]
pub struct CallFrame {
    pub closure_id: ObjId, // object id as pointer into the Heap datastructure
    pub ip: usize,
    pub slots: usize, // offset
}

impl CallFrame {
    /// this is required to know if the operand to an opcode is the
    /// next byte or the next three bytes (lots of constants in chunks.)
    pub fn is_long(&self, heap: &Heap) -> bool {
        match heap.get(self.closure_id).value {
            GcValue::Closure(ref closure) => self.ip >= closure.function.chunk.index_const24,
            _ => panic!("Expected to find a closure"),
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "fn {} \n {}",
            match &self.name {
                Some(s) => s,
                None => "Script",
            },
            self.chunk
        )
    }
}

impl Default for Function {
    fn default() -> Self {
        Self::new()
    }
}

impl Function {
    pub const fn new() -> Self {
        Self {
            arity: 0,
            name: None,
            chunk: Chunk::new(),
            upvalue_count: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FunctionType {
    Function,
    Method,
    #[default]
    Script,
}
