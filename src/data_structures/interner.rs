use std::sync::{LazyLock, Mutex};
use string_interner::{
    StringInterner,
    backend::StringBackend,
    symbol::SymbolU32,
};

type Interner = LazyLock<Mutex<StringInterner<StringBackend>>>;
pub(crate) static STRING_INTERNER: Interner = LazyLock::new(|| Mutex::new(StringInterner::default()));

pub fn intern(string: &str) -> SymbolU32 {
    STRING_INTERNER.lock().unwrap().get_or_intern(string)
}

pub fn get_string(symbol: SymbolU32) -> Option<String> {
    STRING_INTERNER.lock().unwrap().resolve(symbol).map(str::to_owned)
}