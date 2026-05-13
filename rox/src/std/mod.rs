pub mod lox_errors;

use crate::{core::value::Value, std::lox_errors::VmError};
use std::time::{SystemTime, UNIX_EPOCH};

pub type VmResult = Result<Value, VmError>;

// the compiler uses this at comptime to know if a named variable is
// a call to a native function
pub fn is_native_call(func_name: &str) -> bool {
    [
        "time::clock",
        "math::max",
        "math::sqrt",
        "math::pow",
        "io::readLine",
        "io::readNumber",
        "strings::str_cmp",
        "strings::str_len",
        "utils::list_len",
    ]
    .contains(&func_name)
}

fn validate_args(arg_count: usize, args: &[Value]) -> VmResult {
    match args.len().checked_sub(arg_count) {
        Some(i) => Ok(Value::Index(i)),
        None => Err(VmError::Runtime("Stack underflow".to_string())),
    }
}

pub mod time {
    use super::*;
    pub fn clock(_arg_count: usize, _args: &[Value]) -> VmResult {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => Ok(Value::Number(d.as_secs_f64())),
            Err(e) => Err(VmError::Native(e.to_string())),
        }
    }
}

pub mod math {
    use super::*;

    pub fn sqrt(arg_count: usize, nums: &[Value]) -> VmResult {
        // let index = nums.len() - 1 - arg_count;
        // we would need somethin similar to get the args to the function
        // since we only need onw here. No need for this.
        if arg_count != 1 {
            return Err(VmError::Runtime("Expects only 1 argument.".to_string()));
        }

        let v = validate_args(arg_count, nums)?;
        let start: usize = Value::as_sizet(&v);

        if let Value::Number(double) = nums[start] {
            Ok(Value::Number(double.sqrt()))
        } else {
            Err(VmError::Runtime("Expects a double(f64).".to_string()))
        }
    }

    pub fn pow(arg_count: usize, nums: &[Value]) -> VmResult {
        let v = validate_args(arg_count, nums)?;
        let start: usize = Value::as_sizet(&v);

        match (&nums[start], &nums[start + 1]) {
            (Value::Number(p), Value::Number(q)) => Ok(Value::Number(p.powf(*q))),
            _ => Err(VmError::Runtime("Expected type number.".to_string())),
        }
    }

    pub fn max(arg_count: usize, nums: &[Value]) -> VmResult {
        let v = validate_args(arg_count, nums)?;
        let start: usize = Value::as_sizet(&v);

        match (&nums[start], &nums[start + 1]) {
            (Value::Number(p), Value::Number(q)) => Ok(Value::Number(p.max(*q))),
            _ => Err(VmError::Runtime("Expected type number.".to_string())),
        }
    }
}

pub mod io {
    use std::io;

    use crate::data_structures::interner;

    use super::*;

    pub fn read_line(_arg_count: usize, _args: &[Value]) -> VmResult {
        match read() {
            Ok(buffer) => {
                let symbol = interner::intern(buffer.trim());
                Ok(Value::String(symbol))
            }
            Err(e) => Err(VmError::Native(e.to_string())),
        }
    }

    fn read() -> Result<String, std::io::Error> {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer)?;
        Ok(buffer)
    }

    pub fn read_number(_arg_count: usize, _args: &[Value]) -> VmResult {
        match read() {
            Ok(s) => match s.parse::<f64>() {
                Ok(num) => Ok(Value::Number(num)),
                Err(e) => Err(VmError::Native(e.to_string())),
            },
            Err(e) => Err(VmError::Native(e.to_string())),
        }
    }
}

pub mod files {}

pub mod strings {
    use crate::data_structures::interner;

    use super::*;

    pub fn str_len(arg_count: usize, args: &[Value]) -> VmResult {
        let v = validate_args(arg_count, args)?;
        let start: usize = Value::as_sizet(&v);

        if let Value::String(symbol) = args[start] {
            let s = interner::get_string(symbol).unwrap();
            return Ok(Value::Number(s.len() as f64));
        } else {
            Err(VmError::Native(
                "String length only computable for strings.".to_string(),
            ))
        }
    }

    pub fn str_cmp(arg_count: usize, args: &[Value]) -> VmResult {
        // NOTE: unlike other functions where the order is irrelevant.
        // the return value is like
        let v = validate_args(arg_count, args)?;
        let start: usize = Value::as_sizet(&v);

        match (&args[start], &args[start + 1]) {
            (Value::String(s_1), Value::String(s_2)) => {
                match (interner::get_string(*s_1), interner::get_string(*s_2)) {
                    (Some(sl), Some(sr)) => Ok(Value::Number((sl.cmp(&sr) as i8) as f64)),
                    _ => Err(VmError::Native(
                        "One of the Strings passed in does not exist.".to_string(),
                    )),
                }
            }
            _ => Err(VmError::Native(
                "string compare expected type of Strings.".to_string(),
            )),
        }
    }
}

pub mod utils {
    use super::*;

    // utility method for lox, to get length of strings, lists etc.
    pub fn list_len(arg_count: usize, args: &[Value]) -> VmResult {
        let v = validate_args(arg_count, args)?;
        let start: usize = Value::as_sizet(&v);

        if let Value::String(symbol) = args[start] {
            return Ok(Value::Nil);
        } else {
            Err(VmError::Native(
                "String length only computable for strings.".to_string(),
            ))
        }
    }
}
