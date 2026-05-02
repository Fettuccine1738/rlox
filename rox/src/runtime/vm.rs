#![allow(unused)]
use core::panic;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::ops::{Add, Div, Mul, Sub};
use std::rc::Rc;

use string_interner::Symbol;
use string_interner::symbol::{self, SymbolU32};

use crate::compile::compiler::Compiler;
use crate::core::chunk::Chunk;
use crate::core::opcode::OpCode;
use crate::core::value::{NativeFn, ObjId, Value};
use crate::data_structures::interner::{self};
use crate::data_structures::map::HashTable;
use crate::runtime::gc::Trace;
use crate::runtime::heap::{
    GcObject, GcValue, Heap, LoxClass, LoxClosure, LoxInstance, UpValueState,
};
use crate::runtime::lang::CallFrame;
use crate::runtime::lang::Function;
use crate::std::{io, math, strings, time};

pub const DEBUG_TRACE: bool = false;
pub const FRAMES_MAX: usize = 64;
pub const STACK_MAX: usize = 256; // update to FRAMES_MAX * UINT8_COUNT
pub const INIT: &str = "init"; // update to FRAMES_MAX * UINT8_COUNT

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum InterpretResult {
    Ok,
    CompileError,
    RuntimeError,
}

pub struct VM {
    pub stack: Vec<Value>,
    pub globals: HashTable,
    pub call_frames: Vec<CallFrame>,
    // when a varialbe moves to the heap, all closures capturing that variable
    // retain a reference  to its one new location. That way when th variable is mutated
    // all closures see the change.
    pub open_upvalues: HashMap<usize, ObjId>,
    heap: Heap,             // dummy: *mut * mut Value
    init_symbol: SymbolU32, // `this` keyword
}

impl Trace for VM {
    fn trace(&self, heap: &mut super::heap::Heap) {
        todo!()
    }
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn init(&mut self) {
        self.reset_stack();
    }

    pub fn new() -> Self {
        Self {
            // chunk: None, // we don't always start out with valid chunks
            stack: Vec::with_capacity(STACK_MAX),
            globals: HashTable::new(),
            call_frames: Vec::with_capacity(FRAMES_MAX),
            open_upvalues: HashMap::new(),
            heap: Heap::new(super::gc::GcMode::Stress),
            init_symbol: interner::intern(INIT),
        }
    }

    fn reset_stack(&mut self) {
        self.stack.clear();
    }

    pub fn interpret(&mut self, source: String) -> InterpretResult {
        match Compiler::compile(&source) {
            None => InterpretResult::CompileError,
            Some(func) => {
                #[cfg(feature = "")]
                println!("{}", func.chunk);
                self.define_native("io::readNumber".to_owned(), NativeFn(io::read_number));
                self.define_native("io::readLine".to_owned(), NativeFn(io::read_line));
                self.define_native("time::clock".to_owned(), NativeFn(time::clock));
                self.define_native("math::sqrt".to_owned(), NativeFn(math::sqrt));
                self.define_native("math::max".to_owned(), NativeFn(math::max));
                self.define_native("math::pow".to_owned(), NativeFn(math::pow));
                self.define_native("strings::str_cmp".to_owned(), NativeFn(strings::str_cmp));

                // guard against garbage collection.
                self.stack.push(Value::LoxFunction(func.clone()));
                self.stack.pop();
                //---
                let func_clone: Rc<Function> = Rc::clone(&func);
                let cloj_id = self.heap.alloc_closure(LoxClosure {
                    function: func.clone(),
                    upvalues: vec![],
                    upvalue_count: 0,
                });
                self.stack.push(Value::Object(cloj_id));
                self.call(&func, ObjId(0), 0);
                self.run()
            }
        }
    }

    pub fn push_value(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    fn peek(&mut self, distance: usize) -> Value {
        self.stack[self.stack.len() - 1 - distance].clone()
    }

    fn get_current_frame_mut(&mut self) -> &mut CallFrame {
        self.call_frames.last_mut().unwrap()
    }

    // short lived calls to please the borrow checker.
    fn get_current_frame(&self) -> &CallFrame {
        self.call_frames.last().unwrap()
    }

    fn current_chunk(&self) -> &Chunk {
        let frame = &self.get_current_frame().closure_id;
        let object = &self.heap.get(*frame).value;
        if let GcValue::Closure(lox) = object {
            &lox.function.chunk
        } else {
            panic!("");
        }
    }

    fn get_frame_closure(&self, closure_id: ObjId) -> &LoxClosure {
        let object = &self.heap.get(closure_id).value;
        if let GcValue::Closure(lox) = object {
            lox
        } else {
            panic!("");
        }
    }

    fn run(&mut self) -> InterpretResult {
        #[cfg(feature = "")]
        if DEBUG_TRACE {
            for v in &self.stack {
                if let Value::Object(id) = v {
                    println!("{} => {:?}", v, self.heap.get(*id));
                } else {
                    println!("{}", v);
                }
            }
        }

        loop {
            #[cfg(feature = "")]
            if DEBUG_TRACE {
                let start = self.get_current_frame().ip;
                Chunk::disassemble_instruction(self.current_chunk(), start);
            }

            // short lived borrows because borrow checker complains about
            // when explicitly borrowed let s = last();
            // self.get_current_frame_mut().ip += 1; // point to the next instruction to read.
            let instruction: OpCode = OpCode::try_from(self.read_byte()).expect("");

            match instruction {
                OpCode::Return => {
                    if let Some(result) = self.stack.pop() {
                        let frame = self.call_frames.pop().unwrap();
                        let base = frame.slots;
                        // close any open upvalues that point into this frame's stack
                        self.close_upvalues(base);
                        // truncate frame back to where this frame started
                        self.stack.truncate(base);

                        if self.call_frames.is_empty() {
                            return InterpretResult::Ok;
                        }
                        self.stack.push(result);
                    } else {
                        return InterpretResult::RuntimeError;
                    }
                }
                OpCode::Invoke => {
                    let name = self.read_string().unwrap();
                    let arg_count = self.read_byte();
                    if !self.invoke(name, arg_count) {
                        return InterpretResult::RuntimeError;
                    }
                }
                OpCode::Constant => {
                    let constant: Value = self.read_constant();
                    self.stack.push(constant);
                }
                OpCode::Constant24 => {
                    let constant: Value = self.read_constant();
                    self.stack.push(constant);
                }
                OpCode::Negate => {
                    if !Value::is_number(&self.stack[self.stack.len() - 1]) {
                        self.runtime_error("Operand must be a number.");
                        return InterpretResult::RuntimeError;
                    } else {
                        let num_value = self.stack.pop().unwrap();
                        self.stack.push((-num_value).unwrap());
                    }
                }
                OpCode::Add
                | OpCode::Divide
                | OpCode::Multiply
                | OpCode::Subtract
                | OpCode::Greater
                | OpCode::Less => {
                    // Clox uses peek here to guard against gc, but this is fine for us because
                    // collection cannot be triggered here.
                    let rhs = self.stack.pop().unwrap();
                    let lhs = self.stack.pop().unwrap();
                    match Self::binary_op(lhs, rhs, instruction) {
                        Some(result) => self.stack.push(result),
                        None => return InterpretResult::RuntimeError,
                    }
                }
                OpCode::NIL => self.stack.push(Value::Nil),
                OpCode::True => self.stack.push(Value::Boolean(true)),
                OpCode::False => self.stack.push(Value::Boolean(false)),
                OpCode::Not => {
                    let value: bool = if let Some(v) = self.stack.pop() {
                        v.is_falsey()
                    } else {
                        return InterpretResult::RuntimeError;
                    };
                    self.stack.push(Value::Boolean(value));
                }
                OpCode::Equal => {
                    let p = self.stack.pop();
                    let q = self.stack.pop();
                    let eq = match (p, q) {
                        (Some(a), Some(b)) => Value::values_equal(a, b),
                        _ => panic!("expected two operands to binary op == "),
                    };
                    self.stack.push(Value::Boolean(eq));
                }
                OpCode::Print => {
                    let value = self.stack.pop().unwrap();
                    println!("{}", value);
                }
                OpCode::Pop => {
                    let _ = self.stack.pop();
                }
                OpCode::PopN => {
                    // simple optimization to pop all elements at once.
                    let n: u8 = self.read_byte();
                    self.stack.truncate(n as usize);
                }
                OpCode::DefineGlobal => {
                    // used to store the global Variable and Value pairs.
                    let name = self.read_string().unwrap();
                    // NOTE: Value is not popped directly off the stack.
                    // This is to ensure that the VM can still find the value after/during garbage collection.b
                    let value = self.peek(0);
                    self.globals.insert(name, value);
                    self.stack.pop(); // value is associated with this variable and not needed on the stack. access with variable name
                }
                OpCode::GetGlobal => {
                    let name = self.read_string().unwrap();
                    let value: Value = match self.globals.get(name) {
                        Some(value) => value,
                        None => return InterpretResult::RuntimeError,
                    };
                    self.stack.push(value);
                }
                OpCode::SetGlobal => {
                    let symbol: SymbolU32 = self.read_string().unwrap();
                    let current: Value = self.peek(0);

                    // Throw RuntimeError if assignment to an undeclared global variable.
                    // insert returns true if a no previous value was declared with this variable name.
                    // false otherwise.
                    if self.globals.insert(symbol, current) {
                        self.globals.delete(symbol);
                        Self::runtime_error(
                            self,
                            format!(
                                "Undefinded Variable  '{}'.",
                                interner::get_string(symbol).unwrap()
                            )
                            .as_str(),
                        );
                        return InterpretResult::RuntimeError;
                    }
                }
                OpCode::GetLocal => {
                    // reads the current frame slots array
                    // which meant it accessed the given numbered slot relative to the beginning of that frame.
                    let offset = self.read_byte();
                    let base: usize = self.get_current_frame_mut().slots;
                    let value = self.read_local_slot(base + (offset as usize));
                    self.push_value(value);
                }
                OpCode::SetLocal => {
                    let slot = self.read_byte();
                    let base: usize = self.get_current_frame_mut().slots;
                    let value: Value = self.peek(0);
                    self.write_local_slot(base + slot as usize, value);
                }
                OpCode::JumpIfFalse => {
                    let offset = self.read_short();
                    if self.peek(0).is_falsey() {
                        self.get_current_frame_mut().ip += offset as usize;
                    }
                }
                OpCode::Jump => {
                    let offset = self.read_short();
                    self.get_current_frame_mut().ip += offset as usize;
                }
                OpCode::Loop => {
                    let offset = self.read_short();
                    self.get_current_frame_mut().ip -= offset as usize;
                }
                OpCode::Call => {
                    let arity = self.read_byte(); // only 256 args allowed, hence read_byte()
                    let function = self.peek(arity as usize);
                    if !self.call_value(function, arity) {
                        return InterpretResult::RuntimeError;
                    }
                    // good time to collect garbage from a function
                    self.collect_garbage();
                }
                OpCode::Closure => {
                    let value = self.read_constant();
                    let function = Value::as_function(&value);
                    let count = function.upvalue_count;
                    let mut upval_ids = vec![ObjId(0); count];

                    for item in upval_ids.iter_mut().take(count) {
                        // for i in 0..count {
                        // encoding [is_long (0 | 1)][(is_long == 0) ? idx_1b : idx_3b][is_local]
                        let is_long = self.read_byte();
                        let index = if is_long == 1 {
                            let mut arr: [u8; 3] = [255, 255, 255];
                            // let bytes = self.read_3_bytes(&mut arr);
                            self.read_3_bytes(&mut arr);
                            Chunk::inverse_resolve(arr[0], arr[1], arr[2])
                        } else {
                            self.read_byte() as usize
                        };

                        let is_local: bool = self.read_byte() == 1;
                        let slot_offset = self.get_current_frame().slots;
                        *item = if is_local {
                            self.capture_upvalue(slot_offset + index)
                        } else {
                            // inherit upvalue from enclosing closure - same ObjId
                            let id = self.get_current_frame().closure_id;
                            self.get_frame_closure(id).upvalues[index]
                        };
                    }

                    let closure = LoxClosure {
                        function: Rc::clone(&function),
                        upvalues: upval_ids,
                        upvalue_count: count,
                    };
                    // allocate closure on heap, push ObjId onto stack
                    let id = self.heap.alloc_closure(closure);
                    self.stack.push(Value::Object(id));
                }
                OpCode::GetUpValue => {
                    // operand is the index into the current function's upvalue array.
                    let slot = self.read_byte() as usize;
                    let id = self.get_current_frame().closure_id;
                    let id: ObjId = self.get_frame_closure(id).upvalues[slot];
                    let value: Value = self.heap.get_upvalue(id, &self.stack);
                    self.stack.push(value);
                }
                OpCode::SetUpValue => {
                    let slot = self.read_byte() as usize;
                    let peek_value = self.peek(0);
                    let id = self.get_current_frame().closure_id;
                    let upval_id: ObjId = self.get_frame_closure(id).upvalues[slot];
                    // assignment is an expression in Lox. so the assigned value remains on the stack.
                    self.heap.set_upvalue(upval_id, peek_value, &mut self.stack);
                }
                OpCode::CloseUpValue => {
                    let slot = self.stack.len() - 1;
                    self.close_upvalues(slot);
                    self.stack.pop();
                }
                OpCode::Class => {
                    let name = interner::get_string(self.read_string().unwrap()).unwrap();
                    let clazz = GcObject::new(GcValue::Class(LoxClass::new(name)));
                    let id = self.heap.alloc(clazz);
                    self.stack.push(Value::Object(id));
                }
                OpCode::GetProperty => {
                    if let Value::Object(id) = self.peek(0) {
                        // again, dribble to bypass big BC!!
                        let property: SymbolU32 = self.read_string().unwrap();
                        let field = interner::get_string(property).unwrap();

                        if let GcValue::Instance(li) = &self.heap.get(id).value {
                            // fields have priority over and shadow methods, hence search first
                            if let Some(value) = li.get_field(property) {
                                // pop instance off the stack and replace with the gotten field
                                self.stack.pop();
                                self.push_value(value);
                            } else {
                                // if this instance does not have a field with the property name,
                                // look for a method in its class.
                                if !self.bind_method(li.class, property) {
                                    let msg = format!("Undefined property access `{}`.", field);
                                    self.runtime_error(&msg);
                                    return InterpretResult::RuntimeError;
                                }
                            }
                        } else {
                            self.runtime_error("Only instances have properties");
                            return InterpretResult::RuntimeError;
                        }
                    } else {
                        self.runtime_error("Only instances have properties");
                        return InterpretResult::RuntimeError;
                    }
                }
                OpCode::SetProperty => {
                    if let Value::Object(id) = self.peek(1) {
                        // NOTE: how we are looking at depth 1, because 0 is the field
                        let field: SymbolU32 = self.read_string().unwrap();
                        let v = self.peek(0);

                        if let GcValue::Instance(li) = &mut self.heap.get_mut(id).value {
                            li.set_field(field, v);
                            // set property is an expression, so we leave the value on the stack but
                            // remove the instance
                            let val = self.pop().unwrap();
                            let _ = self.pop(); // remove settee
                            self.push_value(val); // push setter
                        } else {
                            self.runtime_error("Only instances have properties");
                            return InterpretResult::RuntimeError;
                        }
                    } else {
                        self.runtime_error("Only instances have properties");
                        return InterpretResult::RuntimeError;
                    }
                }
                OpCode::Method => {
                    let name = self.read_string().unwrap();
                    self.define_method(name);
                }
                OpCode::Inherit => {
                    if let Value::Object(super_id) = self.peek(1) {
                        // clox checks at here if a super is a class but we defer it to the heap
                        if let Value::Object(sub_id) = self.peek(0) {
                            // the subclass at this point is empty, so when the subclass's methods
                            // are added, overwritten methods shadow the superclass's methods.
                            if !self.heap.orchestrate_inherit(super_id, sub_id) {
                                self.runtime_error("Superclass must be a class.");
                            }
                            self.pop();
                        }
                    } else {
                        self.runtime_error("Superclass must be a class.");
                    }
                }
                OpCode::GetSuper => {
                    let name = self.read_string().unwrap();
                    if let Value::Object(sup_id) = self.pop().unwrap()
                        && !self.bind_method(sup_id, name)
                    {
                        return InterpretResult::RuntimeError;
                    }
                    // else method not required, compiler would have caught this error.
                }
                OpCode::SuperInvoke => {
                    let name = self.read_string().unwrap();
                    let arg_count = self.read_byte();
                    if let Value::Object(sup_id) = self.pop().unwrap()
                        && !self.invoke_from_class(sup_id, name, arg_count)
                    {
                        return InterpretResult::RuntimeError;
                    }
                }
            }
        }
    }

    fn bind_method(&mut self, class: ObjId, name: SymbolU32) -> bool {
        if let GcValue::Class(clazz) = &self.heap.get(class).value {
            // again no need to get as closure, the call() function
            // checks if the Object is either a Class, Closure or Nativefn etc
            clazz.get_method(name).is_some_and(|v| {
                self.push_value(v);
                true
            });
        }
        false
    }

    /// marks all roots with allocations on the heap
    /// as grey
    pub fn collect_garbage(&mut self) {
        let roots: HashSet<ObjId> = self.find_roots();
        self.heap.mark_roots(roots.into_iter());
        self.heap.trace_references();
        self.heap.sweep();
    }

    fn find_roots(&self) -> HashSet<ObjId> {
        let mut objects = HashSet::new();

        for v in &self.stack {
            if let Value::Object(id) = v {
                objects.insert(*id);
            }
        }

        for entry in self.globals.iter() {
            if let Value::Object(id) = entry.get_value() {
                objects.insert(*id);
            }
        }

        for f in &self.call_frames {
            objects.insert(f.closure_id);
        }

        objects
    }

    fn invoke(&mut self, name: SymbolU32, arg_count: u8) -> bool {
        if let Value::Object(recv) = self.peek(arg_count as usize) {
            if let GcValue::Instance(i) = &self.heap.get(recv).value {
                if let Some(v) = i.get_field(name) {
                    // replace instance on the stack with it gotten property
                    let idx = self.stack.len() - arg_count as usize - 1;
                    self.stack[idx] = v.clone(); // inexpensive bounded method call
                    return self.call_value(v, arg_count);
                } else {
                    return self.invoke_from_class(i.class, name, arg_count);
                }
            } else {
                self.runtime_error("Only instances have methods.");
                return false;
            }
        }
        false
    }

    fn invoke_from_class(&mut self, class_id: ObjId, name: SymbolU32, arg_count: u8) -> bool {
        if let GcValue::Class(m) = &self.heap.get(class_id).value {
            if let Some(Value::Object(cloj)) = m.get_method(name) {
                let f = self.heap.get(cloj).as_function().unwrap();
                // receiver and args alread on stack
                return self.call(&f, cloj, arg_count);
            } else {
                let msg = format!("Undefined property {}", interner::get_string(name).unwrap());
                self.runtime_error(&msg);
            }
        }
        false
    }

    fn call_value(&mut self, callee: Value, arity: u8) -> bool {
        if Value::is_object(&callee) {
            return match &callee {
                Value::NativeFunction(func) => {
                    let arg_start = self.stack.len() - arity as usize; // slot 0 irrelevant here, hence no -1
                    let args: &[Value] = &self.stack[arg_start..]; // send only the args the functions need
                    match (func.0)(arity as usize, args) {
                        Ok(result) => {
                            self.stack.truncate(arg_start - 1); // remove function and its arguments.
                            self.push_value(result);
                            return true;
                        }
                        Err(e) => self.runtime_error(&e.to_string()),
                    }
                    false
                }
                Value::Object(id) => {
                    match &self.heap.get(*id).value {
                        GcValue::Closure(clojure) => {
                            let function: Rc<Function> = clojure.function.clone();
                            return self.call(&function, *id, arity);
                        }
                        GcValue::Class(klass) => {
                            let constructor: Option<Value> = klass.get_method(self.init_symbol);
                            let instance: LoxInstance = LoxInstance::new(*id);
                            let new_obj: ObjId =
                                self.heap.alloc(GcObject::new(GcValue::Instance(instance)));
                            // store reference on the stack slot where local 0 would have been
                            let idx = self.stack.len() - arity as usize - 1;
                            self.stack[idx] = Value::Object(new_obj);
                            if let Some(Value::Object(init_id)) = constructor {
                                let function = self.heap.get(init_id).as_function().unwrap();
                                return self.call(&function, init_id, arity);
                            } else if arity != 0 {
                                // when a no-args constructor is (implicitly) defined but constructor is called with args
                                let msg = format!("Expected 0 arguments but got {}", arity);
                                self.runtime_error(&msg);
                            }
                            true
                        }
                        GcValue::Method(m) => {
                            let obj = self.heap.get(m.closure);
                            let function = obj.as_function().unwrap();
                            // place the instance(receiver) of this method where local 0 sits.
                            let idx = self.stack.len() - arity as usize - 1;
                            self.stack[idx] = Value::Object(m.receiver);
                            return self.call(&function, m.closure, arity);
                        }
                        _ => false,
                    }
                }
                _ => false,
            };
        }
        self.runtime_error("Can only call functions, closures and constructors.");
        false
    }

    /// Reads a local slot, using the shared upvalue cell when the slot has been captured.
    fn read_local_slot(&self, index: usize) -> Value {
        if let Some(id) = self.open_upvalues.get(&index) {
            match &self.heap.get(*id).value {
                GcValue::UpValue(UpValueState::Open(slot)) => self.stack[*slot].clone(),
                GcValue::UpValue(UpValueState::Closed(val)) => val.clone(),
                _ => panic!("expected upvalue"),
            }
        } else {
            self.stack[index].clone()
        }
    }

    /// Writes a local slot and keeps any open upvalue for that slot in sync.
    fn write_local_slot(&mut self, index: usize, value: Value) {
        if let Some(id) = self.open_upvalues.get(&index) {
            // read through the heap object
            match self.heap.get_mut(*id).value {
                GcValue::UpValue(UpValueState::Open(slot)) => {
                    self.stack[slot] = value;
                }
                GcValue::UpValue(UpValueState::Closed(ref mut val)) => {
                    *val = value;
                }
                _ => panic!("expected upvalue"),
            }
        } else {
            self.stack[index] = value;
        }
    }

    /// Close all upvalues that point at or above `last` on the stack.
    pub fn close_upvalues(&mut self, last: usize) {
        let slots_to_close: Vec<usize> = self
            .open_upvalues
            .keys()
            .filter(|&&slot| slot >= last)
            .copied()
            .collect();

        for slot in slots_to_close {
            let id = self.open_upvalues.remove(&slot).unwrap();
            let val = self.stack[slot].clone();
            self.heap.get_mut(id).value = GcValue::UpValue(UpValueState::Closed(val));
        }
    }

    /// checks if a closure already captured an UpValue and reuses if true.
    /// else creates a new rumtime representation for it.
    fn capture_upvalue(&mut self, slot: usize) -> ObjId {
        if let Some(&id) = self.open_upvalues.get(&slot) {
            return id;
        }

        let value = GcValue::UpValue(UpValueState::Open(slot));
        let id = self.heap.alloc(GcObject::new(value));
        self.open_upvalues.insert(slot, id);
        id
    }

    // takes name of function and the Funtion ptr
    fn define_native(&mut self, name: String, function: NativeFn) {
        let symbol = interner::intern(&name);
        // done because Garbage collection can be triggered anywhere.
        self.push_value(Value::String(symbol));
        self.push_value(Value::NativeFunction(function));

        self.globals.insert(symbol, self.stack[1].clone());
        self.pop();
        self.pop();
    }

    /// FIX: we already allocated the closure on the heap, ideally
    /// the class should own the Closure / Function.
    /// The objId adds a costly indirection. If a class sits directly above a Class
    /// that hints we need to bind the method to the class and should defer allocation on the heap
    fn define_method(&mut self, name: SymbolU32) {
        // NOTE: Clox uses AS_CLASS(_) methods to check type at runtime
        // If let helps us guard against wrong type use too.. Rust ftw
        if let Value::Object(method_id) = self.peek(0) {
            // method closure sitting on the stack
            // must be a Value::Object()
            // get heap object through reference on the stack
            if let Value::Object(class_id) = self.peek(1)
                && let GcValue::Class(class) = &mut self.heap.get_mut(class_id).value
            {
                class.add_method(name, method_id);
                self.pop(); // remove method object sitting on the stack
            }
        } else {
            let msg = format!("Expected to find Method but found {:?}", self.peek(0));
            self.runtime_error(&msg);
        }
    }

    // closure id is add here in case the frame needs to access the heap
    // to get upvalues
    fn call(&mut self, function: &Rc<Function>, closure_id: ObjId, arity: u8) -> bool {
        if arity != function.arity {
            let err_msg: String =
                format!("Expected {} arguments but got {}", function.arity, arity);
            Self::runtime_error(self, &err_msg);
            return false;
        }

        if self.call_frames.len() == FRAMES_MAX {
            Self::runtime_error(self, "Stack overflow");
            return false;
        }
        // [ fn ] [ arg0 ] [ arg1 ] [ arg2 ]  <-- stackTop
        // ^      | -------args to function ------
        // slots points here (slot 0 = the function being called)
        self.call_frames.push(CallFrame {
            closure_id, // note rc cloned before passing in, use clojure.
            ip: 0,
            slots: self.stack.len() - arity as usize - 1,
        });
        true
    }

    fn runtime_error(&mut self, msg: &str) {
        eprintln!("{}", msg);
        for i in (0..self.call_frames.len()).rev() {
            let frame: &CallFrame = &self.call_frames[i];
            // - 1 because ip points to the next instruction to be executed
            // but the failed instruction was the previous one.
            let instruction: usize = frame.ip - 1;
            let line = self
                .get_frame_closure(frame.closure_id)
                .function
                .chunk
                .lines[instruction];
            let name = &self.get_frame_closure(frame.closure_id).function.name;

            eprint!("[line {}] in ", line.0);
            match &name {
                Some(name) => eprintln!("{}", name),
                None => eprintln!("Script"),
            }
            eprint!("[line {}] in ", line.0);
        }
        self.reset_stack();
    }

    // is_long : when opcode is OP_CONSTANT_LONG: Operand is 24bits.
    fn read_constant(&mut self) -> Value {
        let is_long = self.call_frames.last().unwrap().is_long(&self.heap);
        let index = if is_long {
            let b1 = self.read_byte() as u32;
            let b2 = self.read_byte() as u32;
            let b3 = self.read_byte() as u32;

            b1 | (b2 << 8) | (b3 << 16)
        } else {
            self.read_byte() as u32
        };

        self.current_chunk()
            .constants
            .get(index as usize)
            .expect("Invalid constant index.")
            .clone()
    }

    // reads the 16 bit operand for jump opCodes
    // retrurns a u16
    fn read_short(&mut self) -> u16 {
        // le order
        let b0_7 = self.read_byte() as u16;
        let b8_15 = self.read_byte() as u16;
        b0_7 | b8_15 << 8
    }

    fn read_byte(&mut self) -> u8 {
        let call_frame = self.call_frames.last().unwrap();
        let id = call_frame.closure_id;
        let ip = call_frame.ip;

        let byte_code: u8 = self
            .get_frame_closure(id)
            .function
            .chunk
            .code
            .get(ip)
            .copied()
            .unwrap();

        self.call_frames.last_mut().unwrap().ip += 1;
        byte_code
    }

    fn read_3_bytes(&mut self, buffer: &mut [u8; 3]) {
        let (ip, closure_id) = {
            let frame = self.call_frames.last_mut().unwrap();
            let ip = frame.ip;
            frame.ip += 3;
            (ip, frame.closure_id)
        };

        let func = self.get_frame_closure(closure_id).function.clone();
        buffer.copy_from_slice(&func.chunk.code[ip..ip + 3]);
    }

    fn binary_op(lhs: Value, rhs: Value, opcode: OpCode) -> Option<Value> {
        match opcode {
            OpCode::Add => lhs.add(rhs),
            OpCode::Divide => lhs.div(rhs),
            OpCode::Multiply => lhs.mul(rhs),
            OpCode::Subtract => lhs.sub(rhs),
            OpCode::Greater => Value::greater_than(&lhs, &rhs),
            OpCode::Less => Value::less_than(&lhs, &rhs),
            _ => None,
        }
    }

    fn read_string(&mut self) -> Option<SymbolU32> {
        // Because we have 2 constant-indexing Operands OpConstant and OpConstant24
        // We need to resolve what operand was used to store this constant.
        // so we know to read either the next byte or next 3 bytes.
        match self.read_constant() {
            Value::String(symbol) => Some(symbol), // interner::get_string(symbol),
            _ => None,
        }
    }
}
