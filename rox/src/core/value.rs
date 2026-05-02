#![allow(unreachable_patterns)]
use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Neg, Sub},
    rc::Rc,
};

use string_interner::symbol::SymbolU32;

use crate::{
    data_structures::interner::{self},
    runtime::lang::Function,
    std::VmResult,
};

/// Java-style reference id to an object stored on the heap.
/// the inner field is the objects index on the Heap managed by the GC.
/// this allows mulitple owners using this id to modify / read the object's content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd)]
pub struct ObjId(pub usize);

impl crate::runtime::gc::Trace for ObjId {
    fn trace(&self, heap: &mut crate::runtime::heap::Heap) {
        heap.mark_object(*self);
    }
}

// stub struct
#[derive(Debug, PartialEq, PartialOrd)]
pub struct Closure;

/// A tagged Union: A value contains 2 parts: a type "tag" and a
/// payload for the actual value.
/// covers kind of values that has built-in-support in the VM.
#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub enum Value {
    Boolean(bool),
    #[default]
    Nil,
    Number(f64),
    LoxFunction(Rc<Function>),
    // interned strings allow us to compare addreses(symbols) which is more efficient
    // than comparing the values(contents) of the strings themselves.
    String(SymbolU32),
    NativeFunction(NativeFn),
    Object(ObjId), // pointer into the GC Heap
}

impl Value {
    // we could do the same for strings, but we already have native functions for that.
    pub fn less_than(lhs: &Value, rhs: &Value) -> Option<Value> {
        match (lhs, rhs) {
            (Value::Number(ln), Value::Number(rn)) => Some(Value::Boolean(ln < rn)),
            _ => None,
        }
    }

    pub fn greater_than(lhs: &Value, rhs: &Value) -> Option<Value> {
        match (lhs, rhs) {
            (Value::Number(ln), Value::Number(rn)) => Some(Value::Boolean(ln > rn)),
            _ => None,
        }
    }

    pub fn is_bool(value: &Value) -> bool {
        matches!(value, Value::Boolean(_))
    }

    pub fn is_nil(value: &Value) -> bool {
        matches!(value, Value::Nil)
    }

    pub fn is_number(value: &Value) -> bool {
        matches!(value, Value::Number(_))
    }

    pub fn is_native(value: &Value) -> bool {
        matches!(value, Value::NativeFunction(_))
    }

    pub fn is_object(value: &Value) -> bool {
        matches!(value, Value::LoxFunction(_))
            || matches!(value, Value::Object(_))
            || matches!(value, Value::NativeFunction(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    pub fn as_bool(value: &Value) -> bool {
        if let Value::Boolean(b) = value {
            *b
        } else {
            panic!("Expected Variant boolean but got {:?}", value);
        }
    }

    pub fn as_native(value: &Value) -> Option<NativeFn> {
        if let Value::NativeFunction(f) = value {
            Some(*f)
        } else {
            None
        }
    }

    pub fn as_number(value: &Value) -> f64 {
        if let Value::Number(n) = value {
            *n
        } else {
            panic!("Expected Variant boolean but got {:?}", value);
        }
    }

    pub fn as_function(value: &Value) -> Rc<Function> {
        if let Value::LoxFunction(boxed_f) = value {
            boxed_f.clone()
        } else {
            panic!("Expected Variant boolean but got {:?}", value);
        }
    }

    pub fn as_object(value: &Value) -> Value {
        Self::is_object(value);
        match value {
            Self::LoxFunction(_) => Value::LoxFunction(Value::as_function(value)),
            Self::Object(o) => Value::Object(*o),
            _ => panic!("Value::Obj expected but got"),
        }
    }

    pub fn values_equal(a: Value, b: Value) -> bool {
        match (a, b) {
            (Value::Boolean(av), Value::Boolean(bv)) => av == bv,
            (Value::Nil, Value::Nil) => true,
            (Value::Number(av), Value::Number(bv)) => av == bv,
            (Value::String(lsz), Value::String(rsz)) => lsz == rsz,
            (Value::Nil, _) => false, // allow java style value != null.
            (_, Value::Nil) => false,
            (Value::Object(id), Value::Object(oid)) => id == oid,
            _ => false,
        }
    }

    // falsiness handles how other types are negated('not'ed)
    // e.g !nil, !"string"
    pub fn is_falsey(&self) -> bool {
        Value::is_nil(self) || (Value::is_bool(self) && !Value::as_bool(self))
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::Nil => write!(f, "nil"),
            Value::String(id) => {
                let s = interner::get_string(*id).unwrap();
                write!(f, "{}", s)
            }
            Value::NativeFunction(n) => write!(f, "{}", n),
            Value::LoxFunction(n) => match &n.name {
                Some(name) => write!(f, "<fn {}>", name),
                None => write!(f, "<script>"),
            },
            Value::Object(o) => write!(f, "Object@{}", o.0),
            _ => todo!(),
        }
    }
}

// standard arithmetic operators implementation for Value
impl Neg for Value {
    type Output = Option<Self>;

    fn neg(self) -> Self::Output {
        match self {
            Value::Boolean(_) => None,
            Value::Number(n) => Some(Value::Number(-n)),
            Value::Nil => Some(Value::Nil),
            _ => None,
        }
    }
}

impl Add for Value {
    type Output = Option<Self>;

    fn add(self, other: Self) -> Self::Output {
        match (&self, &other) {
            (Value::Number(l), Value::Number(r)) => Some(Value::Number(l + r)),
            (Value::String(lhs), Value::String(rhs)) => {
                let l_str = interner::get_string(*lhs);
                let r_str = interner::get_string(*rhs);
                match (l_str, r_str) {
                    (Some(mut l), Some(r)) => {
                        l.push_str(&r); // NOTE: we are not consuming rhs.
                        let symbol = interner::intern(&l);
                        Some(Value::String(symbol))
                    }
                    _ => None,
                }
            }
            // String concatenation: This needed for print statments.
            (Value::String(lhs), Value::Number(n)) => match interner::get_string(*lhs) {
                Some(mut string) => {
                    string.push_str(&n.to_string());
                    let symbol = interner::intern(&string);
                    Some(Value::String(symbol))
                }
                None => None,
            },
            (Value::Number(n), Value::String(lhs)) => {
                match interner::get_string(*lhs) {
                    Some(string) => {
                        let mut new_string = n.to_string(); // order matters here.
                        new_string.push_str(&string);
                        let symbol = interner::intern(&new_string);
                        Some(Value::String(symbol))
                    }
                    None => None,
                }
            }
            _ => None,
        }
    }
}

impl Div for Value {
    type Output = Option<Self>;

    fn div(self, other: Self) -> Self::Output {
        match (&self, &other) {
            (Value::Number(l), Value::Number(r)) => Some(Value::Number(l / r)),
            _ => None,
        }
    }
}

impl Mul for Value {
    type Output = Option<Self>;

    fn mul(self, other: Self) -> Self::Output {
        match (&self, &other) {
            (Value::Number(l), Value::Number(r)) => Some(Value::Number(l * r)),
            _ => None,
        }
    }
}

impl Sub for Value {
    type Output = Option<Self>;

    fn sub(self, other: Self) -> Self::Output {
        match (&self, &other) {
            (Value::Number(l), Value::Number(r)) => Some(Value::Number(l - r)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialOrd)]
#[allow(unpredictable_function_pointer_comparisons)]
pub struct NativeFn(pub for<'a> fn(usize, &'a [Value]) -> VmResult);

impl Display for NativeFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native fn>")
    }
}

impl PartialEq for NativeFn {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::fn_addr_eq(self.0, other.0)
    }
}
