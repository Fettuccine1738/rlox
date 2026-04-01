pub mod lox_errors;

use crate::{core::value::Value, std::lox_errors::VmError};
use std::time::{SystemTime, UNIX_EPOCH};

pub type VmResult = Result<Value, VmError>;

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

        let index = nums
            .len()
            .checked_sub(arg_count)
            .ok_or_else(|| VmError::Runtime("Stack underflow".to_string()))?;

        if let Value::Number(double) = nums[index] {
            Ok(Value::Number(double.sqrt()))
        } else {
            Err(VmError::Runtime("Expects a double(f64).".to_string()))
        }
    }

    pub fn pow(arg_count: usize, nums: &[Value]) -> VmResult {
        // arity checking.
        let start = nums
            .len()
            .checked_sub(arg_count)
            .ok_or_else(|| VmError::Runtime("Stack underflow".to_string()))?;

        match (&nums[start], &nums[start + 1]) {
            (Value::Number(p), Value::Number(q)) => Ok(Value::Number(p.powf(*q))),
            _ => Err(VmError::Runtime("Expected type number.".to_string())),
        }
    }
    pub fn max(arg_count: usize, nums: &[Value]) -> VmResult {
        // arity checking.
        let start = nums
            .len()
            .checked_sub(arg_count)
            .ok_or_else(|| VmError::Runtime("Stack underflow".to_string()))?;

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

    pub fn read_line(arg_count: usize, args: &[Value]) -> VmResult {
        let _start = args
            .len()
            .checked_sub(arg_count)
            .ok_or_else(|| VmError::Runtime("Stack underflow".to_string()))?;

        match read() {
            Ok(buffer) => {
                let symbol = interner::intern(&buffer.trim());
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

    pub fn read_number(arg_count: usize, args: &[Value]) -> VmResult {
        let _start = args
            .len()
            .checked_sub(arg_count)
            .ok_or_else(|| VmError::Runtime("Stack underflow".to_string()))?;

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

    pub fn str_cmp(arg_count: usize, args: &[Value]) -> VmResult {
        // NOTE: unlike other functions where the order is irrelevant.
        // the return value is like
        let start = args
            .len()
            .checked_sub(arg_count)
            .ok_or_else(|| VmError::Runtime("Stack underflow".to_string()))?;

        match (&args[start], &args[start + 1]) {
            (Value::String(s_1), Value::String(s_2)) => {
                match (interner::get_string(*s_1), interner::get_string(*s_2)) {
                    (Some(sl), Some(sr)) => {
                        return Ok({
                            Value::Number((sl.cmp(&sr) as i8) as f64)
                            // if sl < sr {
                            //     Value::Number(-1.0)
                            // } else if sl > sr {
                            //     Value::Number(1.0)
                            // } else {
                            //     Value::Number(0.0)
                            // }
                        });
                    }
                    _ => {
                        return Err(VmError::Native(
                            "One of the Strings passed in does not exist.".to_string(),
                        ));
                    }
                }
            }
            _ => {
                return Err(VmError::Native(
                    "string compare expected type of Strings.".to_string(),
                ));
            }
        }
    }
}
