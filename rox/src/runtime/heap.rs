#![allow(unused)]
use std::fmt::Display;
use std::rc::Rc;

use crate::core::value::ObjId;
use crate::core::value::Value;
use crate::data_structures::map::HashTable;
use crate::runtime::gc::GcMode;
use crate::runtime::gc::Trace;
use crate::runtime::lang::CallFrame;
use crate::runtime::lang::Function;
use crate::runtime::vm::VM;

use rlox_gc_derive::Trace;

/// Next threshold that triggers gc collection
const GC_THRESHOLD: usize = 1024 * 1024;

#[derive(Debug, Clone, Trace)]
pub struct GcObject {
    pub value: GcValue,
    #[unsafe_ignore_trace]
    is_marked: bool,
}

impl GcObject {
    pub fn new(gcvalue: GcValue) -> Self {
        Self {
            value: gcvalue,
            is_marked: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LoxInstance;
#[derive(Debug, Clone, Copy)]
pub struct BoundMethod;
#[derive(Debug, Clone, Copy)]
pub struct LoxClass;
#[derive(Debug, Clone)]
pub struct LoxClosure {
    pub function: Rc<Function>,
    pub upvalues: Vec<ObjId>,
    pub upvalue_count: usize,
}

// NOTE: Tests show its fine to collect closures / functions
// which can no longer be reached. For nested closures see `tests_closures_see_global_mutations`
// Although the LoxClosure object on the heap is collected. The closures still lives in the constant
// table of its enclosing function.
impl Trace for LoxClosure {
    fn trace(&self, heap: &mut super::heap::Heap) {
        // functions are static and will not be collected
        for id in &self.upvalues {
            heap.mark_object(*id);
        }
    }
}

impl Display for LoxClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.function.name.is_none() {
            writeln!(f, "script")
        } else {
            writeln!(
                f,
                "function: {} \n UPVALUES: {:?}\n",
                self.function, self.upvalues
            )
        }
    }
}

/// Open UpValue refer to an upvalue that points to a local variable still on the stack.
/// Closed refers to a variable moved to the Heap.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum UpValueState {
    Open(usize),   // index into the vm's stack.
    Closed(Value), // captured after close_upvalues()
}

#[derive(Debug, Clone, Trace)]
pub enum GcValue {
    #[unsafe_ignore_trace]
    Instance(LoxInstance),
    #[unsafe_ignore_trace]
    BoundMethod(BoundMethod),
    #[unsafe_ignore_trace]
    Class(LoxClass),
    Closure(LoxClosure),
    #[unsafe_ignore_trace]
    UpValue(UpValueState),
}

pub struct Heap {
    pub objects: Vec<Option<GcObject>>,
    pub grey_stack: Vec<ObjId>,
    pub bytes_allocated: usize,
    pub next_gc: usize,
    pub gc_mode: GcMode,
}

impl Heap {
    pub fn new(gc_mode: GcMode) -> Self {
        Self {
            objects: vec![],
            grey_stack: vec![],
            bytes_allocated: 0,
            next_gc: GC_THRESHOLD,
            gc_mode: gc_mode,
        }
    }

    // pub fn mark_roots(&mut self, vm: &VM) {
    //     self.mark_stack(&vm.stack);
    //     self.mark_table(&vm.globals);
    //     self.mark_frames(&vm.call_frames);
    // }

    pub fn mark_roots(&mut self, roots: impl Iterator<Item = ObjId>) {
        for id in roots {
            self.mark_object(id);
        }
    }

    fn mark_stack(&mut self, stack: &[Value]) {
        for v in stack {
            if let Value::Object(id) = v {
                self.mark_object(*id);
            }
        }
    }

    fn mark_table(&mut self, hash_table: &HashTable) {
        for entry in hash_table.iter() {
            if let &Value::Object(id) = entry.get_value() {
                self.mark_object(id);
            }
        }
    }

    fn mark_frames(&mut self, frames: &[CallFrame]) {
        for f in frames {
            let id = f.closure_id;
            self.mark_object(id);
        }
    }

    pub fn mark_object(&mut self, id: ObjId) {
        if self.is_marked(id) {
            return;
        }
        self.set_marked(id, true);
        self.grey_stack.push(id);
    }

    pub fn trace_references(&mut self) {
        while let Some(id) = self.grey_stack.pop() {
            self.blacken_object(id);
        }
    }

    fn blacken_object(&mut self, id: ObjId) {
        let addr = id.0;
        let obj = self.objects[addr].take().unwrap();
        obj.trace(self);
        self.objects[addr] = Some(obj);
    }

    pub fn sweep(&mut self) {
        for slot in self.objects.iter_mut() {
            match slot {
                Some(obj) if obj.is_marked => {
                    obj.is_marked = false; // reset for next cycle
                }
                slot => {
                    match self.gc_mode {
                        GcMode::Log => {
                            let size: usize = std::mem::size_of::<GcObject>();
                            println!(
                                " collected {size} bytes (at {:p} for {:#?}",
                                slot,
                                slot.as_ref()
                            );
                        }
                        _ => (),
                    }
                    *slot = None;
                }
            }
        }
    }

    fn is_marked(&mut self, id: ObjId) -> bool {
        self.objects[id.0]
            .as_ref()
            .map_or(false, |obj| obj.is_marked)
    }

    fn set_marked(&mut self, id: ObjId, marked: bool) {
        if let Some(obj) = self.objects[id.0].as_mut() {
            obj.is_marked = marked;
        }
    }

    pub fn alloc(&mut self, object: GcObject) -> ObjId {
        if self.bytes_allocated > self.next_gc {
            self.collect_garbage();
        }
        let mut size = 0;
        let mut id: usize = 0;
        // Look for an empty slot first (from a previous sweep)
        if let Some(slot) = self.objects.iter().position(|s| s.is_none()) {
            self.objects[slot] = Some(object);
            size = std::mem::size_of::<GcObject>();
            id = slot;
        } else {
            // No free slots, grow the vec
            self.objects.push(Some(object));
            size = std::mem::size_of::<GcObject>();
            id = self.objects.len() - 1;
        }

        self.bytes_allocated += size;
        // NOTE: refactor this, it wastes allocations for other gcmodes.
        let info = format!("allocate {size} bytes {:?}", self.objects[id].as_ref());
        self.gc_mode.info(&info);
        ObjId(id)
    }

    pub fn get_mut(&mut self, id: ObjId) -> &mut GcObject {
        self.objects[id.0]
            .as_mut()
            .expect("attempted to get a swept object")
    }

    pub fn get(&self, id: ObjId) -> &GcObject {
        self.objects[id.0]
            .as_ref()
            .expect("attempted to get a swept object")
    }

    pub fn get_upvalue(&self, id: ObjId, stack: &[Value]) -> Value {
        match &self.get(id).value {
            GcValue::UpValue(UpValueState::Open(slot)) => stack[*slot].clone(),
            GcValue::UpValue(UpValueState::Closed(val)) => val.clone(),
            _ => {
                panic!("Expected upvalue");
            }
        }
    }

    pub fn set_upvalue(&mut self, id: ObjId, new_val: Value, stack: &mut [Value]) {
        let open_slot = match &self.get(id).value {
            GcValue::UpValue(UpValueState::Open(slot)) => Some(*slot),
            _ => None,
        };

        match open_slot {
            Some(slot) => stack[slot] = new_val,
            None => match &mut self.get_mut(id).value {
                GcValue::UpValue(UpValueState::Closed(val)) => {
                    *val = new_val;
                }
                _ => panic!("expected upvalue"),
            },
        }
    }

    pub fn alloc_closure(&mut self, closure: LoxClosure) -> ObjId {
        if self.bytes_allocated > self.next_gc {
            self.collect_garbage();
        }
        self.alloc(GcObject {
            is_marked: false,
            value: GcValue::Closure(closure),
        })
    }

    fn collect_garbage(&mut self) {
        self.gc_mode.start();
        self.gc_mode.end();
    }
}
