use std::sync::{LazyLock, Mutex};
use string_interner::{
    StringInterner,
    backend::StringBackend,
    symbol::{Symbol, SymbolU32},
};

type Interner = LazyLock<Mutex<StringInterner<StringBackend>>>;
pub(crate) static STRING_INTERNER: Interner = LazyLock::new(|| Mutex::new(StringInterner::default()));

pub fn intern(string: &str) -> SymbolU32 {
    let mut interner = STRING_INTERNER.lock().unwrap();
    return interner.get_or_intern(string);
}

pub fn find_string(string: &str) -> usize {
    let mut interner = STRING_INTERNER.lock().unwrap();
    return interner.get_or_intern(string).to_usize();
}

pub fn get_string(symbol: usize) -> Option<String> {
    let mut mutex_guard = STRING_INTERNER.lock().unwrap();
    let optional_string: Option<&str> = match SymbolU32::try_from_usize(symbol) {
        Some(symbol32) => {
            mutex_guard.resolve(symbol32)
        },
        None => None,
    };

    if let Some(s) = optional_string {
        Some(s.to_owned())
    } else {
        None
    }
}