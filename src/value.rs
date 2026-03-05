use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Neg, Sub},
};

/// A tagged Union: A value contains 2 parts: a type "tag" and a
/// payload for the actual value.
/// covers kind of values that has built-in-support in the VM.
#[derive(Debug, Copy, Clone)]
pub enum Value {
    Boolean(bool),
    Nil,
    Number(f64),
}

impl Value {
    pub fn is_bool(value: &Value) -> bool {
        matches!(value, Value::Boolean(_))
    }

    pub fn is_nil(value: &Value) -> bool {
        matches!(value, Value::Nil)
    }

    pub fn is_number(value: &Value) -> bool {
        matches!(value, Value::Number(_))
    }

    pub fn as_bool(value: &Value) -> bool {
        if let Value::Boolean(b) = value {
            *b
        } else {
            panic!("Expected Variant boolean but got {:?}", value);
        }
    }

    pub fn as_number(value: &Value) -> f64 {
        if let Value::Number(n) = value {
            *n
        } else {
            panic!("Expected Variant boolean but got {:?}", value);
        }
    }
}

// #[derive(Debug, Clone, Copy, PartialEq)]
// pub struct Value(pub f64);

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::Nil => write!(f, "{}", *self),
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
        }
    }
}

impl Add for Value {
    type Output = Option<Self>;

    fn add(self, other: Self) -> Self::Output {
        match (&self, &other) {
            (Value::Number(l), Value::Number(r)) => Some(Value::Number(l + r)),
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
