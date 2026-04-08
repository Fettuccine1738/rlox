use crate::core::chunk::Chunk;
use crate::runtime::vm::RtimeUpValue;
use std::fmt::Display;
use std::rc::Rc;
use std::vec;

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
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }

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
        // match self.chunk.partial_cmp(&other.chunk) {
        //     Some(core::cmp::Ordering::Equal) => {}
        //     ord => return ord,
        // }
        self.name.partial_cmp(&other.name)
    }
}
/// A CallFrame represents a single ongoing function call. The slots field
/// points into the VM's value stack at the first slot that this function
/// can use.
pub struct CallFrame {
    pub closure: Rc<Closure>,
    pub ip: usize,
    pub slots: usize, // offset
}

impl CallFrame {
    /// this is required to know if the operand to an opcode is the
    /// next byte or the next three bytes (lots of constants in chunks.)
    pub fn read_long(&self) -> bool {
        self.ip >= self.closure.function.chunk.index_const24
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "fn {}",
            match &self.name {
                Some(s) => s,
                None => "Script",
            }
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

#[derive(Debug, Default, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FunctionType {
    Function,
    #[default]
    Script,
}

/// Different closures may have different number of upvalues.
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct Closure {
    pub function: Rc<Function>,
    pub upvalues: Vec<RtimeUpValue>,
    /// this is stored incase GC cleans up function.
    pub upvalue_count: usize,
}

impl Closure {
    pub fn new(func: Rc<Function>) -> Self {
        let count = func.upvalue_count;
        // per Bob; careful dance to please the garbage collector.
        // let upvalues_init = std::iter::from_fn(|| None)
        //     .take(count)
        //     .collect::<Vec<Option<RtimeUpValue>>>();
        Self {
            function: func,
            upvalues: vec![],
            upvalue_count: count,
        }
    }

    pub fn clone(func: &Rc<Function>) -> Self {
        // let upvalues_init = std::iter::from_fn(|| None)
        //     .take(func.upvalue_count)
        //     .collect::<Vec<Option<RtimeUpValue>>>();
        Self {
            function: func.clone(),
            upvalues: vec![],
            upvalue_count: func.upvalue_count,
        }
    }
}
