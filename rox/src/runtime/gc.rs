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
        if let Self::Log = self {
            println!("{:-^15}", "gc_begin")
        }
    }

    pub fn end(&self) {
        if let Self::Log = self {
            println!("{:-^15}", "gc_end")
        }
    }

    pub fn info(&self, info: &str) {
        if let Self::Log = self {
            println!("{}", info)
        }
    }
}
