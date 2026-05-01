#![allow(unused)]
use std::fmt::Display;
use std::hash::Hash;
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
use string_interner::symbol::SymbolU32;

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

    pub fn is_instance(&self) -> bool {
        matches!(self.value, GcValue::Instance(_))
    }

    pub fn is_closure(&self) -> bool {
        matches!(self.value, GcValue::Closure(_))
    }

    pub fn is_class(&self) -> bool {
        matches!(self.value, GcValue::Class(_))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoundMethod;

/// Classes : are how we create new instances, name required to get instance
/// contain methods: behavior of Instances
#[derive(Debug, Clone)]
pub struct LoxClass {
    name: String, // we can use LoxString here but this is easier
}

impl LoxClass {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

/// From the user’s perspective, an instance of Cake is a different
/// type of object than an instance of Pie. But, from the VM’s
/// perspective, every class the user defines is simply another
/// value of type ObjClass. Likewise, each instance in the user’s
/// program, no matter what class it is an instance of, is an ObjInstance.
#[derive(Debug, Clone)]
pub struct LoxInstance {
    class: ObjId,
    fields: HashTable,
}

impl LoxInstance {
    pub fn new(clazz: ObjId) -> Self {
        // TODO: the fields of an instance are already interned and always
        // return a compact 32 bit symbol. can we use this symbol to index a vec!
        // instead of an hashtable?
        // LIMITATIONS:
        // Hashtable may rehash so there may be a small cost to pay on misses.
        // The symbol are not aligned for access. i.e
        // interner{`Foo`(sybmol = 1),`Bar` (symbol = 2)}  ;; using Bar to index here
        // would mean our first field is Symbol(2), ideally we want to have
        // Symbol(0) as the first.
        Self {
            class: clazz,
            fields: HashTable::new(),
        }
    }

    pub fn get_field(&self, property: SymbolU32) -> Option<Value> {
        self.fields.get(property)
    }

    /// setter implicitly creates the field if it does not exist
    /// therefore guaranteed to always succeed.
    pub fn set_field(&mut self, key: SymbolU32, value: Value) {
        let _ = self.fields.insert(key, value);
    }
}

impl Trace for LoxInstance {
    fn trace(&self, heap: &mut super::heap::Heap) {
        heap.mark_object(self.class);
        heap.mark_table(&self.fields);
    }
}

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
    #[unsafe_ignore_trace] // classes are static and should not be collected.
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
            gc_mode,
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
                    if let GcMode::Log = self.gc_mode {
                        let size: usize = std::mem::size_of::<GcObject>();
                        println!(
                            " collected {size} bytes (at {:p} for {:#?}",
                            slot,
                            slot.as_ref()
                        );
                    }
                    *slot = None;
                }
            }
        }
    }

    fn is_marked(&mut self, id: ObjId) -> bool {
        self.objects[id.0].as_ref().is_some_and(|obj| obj.is_marked)
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
        let size = std::mem::size_of::<GcObject>();
        let mut id: usize = 0;
        // Look for an empty slot first (from a previous sweep)
        if let Some(slot) = self.objects.iter().position(|s| s.is_none()) {
            self.objects[slot] = Some(object);
            id = slot;
        } else {
            // No free slots, grow the vec
            self.objects.push(Some(object));
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
