// Implementation where  Heap is a thread-safe global structure
// and not owned by the VM.
// use crate::runtime::heap::Heap;
// use std::sync::{LazyLock, Mutex};
// type Mutex_Heap = LazyLock<Mutex<Heap>>;
// pub(crate) static GLOBAL_HEAP: Mutex_Heap = LazyLock::new(|| Mutex::new(StringInterner::default()));

pub trait Trace {
    fn trace(&self, heap: &mut super::heap::Heap);
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(u8)]
pub enum GcMode {
    Stress,
    #[default]
    Log,
}

impl GcMode {
    pub fn start(&self) {
        match self {
            Self::Log => println!("{:-^15}", "gc_begin"),
            _ => (),
        }
    }

    pub fn end(&self) {
        match self {
            Self::Log => println!("{:-^15}", "gc_end"),
            _ => (),
        }
    }

    pub fn info(&self, info: &str) {
        match self {
            Self::Log => println!("{}", info),
            _ => (),
        }
    }
}
