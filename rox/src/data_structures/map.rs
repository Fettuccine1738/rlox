use string_interner::{Symbol, symbol::SymbolU32};

use crate::core::value::Value;
use std::fmt::Debug;

/// K: SymbolU32 is the interned string id, the Value::String(SymbolU32)
/// already carries required information, no need duplicating the interened String again.
#[derive(Debug, Clone)]
pub struct HashTable {
    /// Vec<Option<Entry>> is used here for open addressing, i.e (find the next empty spot when keys collide)
    /// Some = occupied,
    /// None = empty slot, terminate probing.
    pub entries: Vec<Option<Entry<SymbolU32, Value>>>,
    pub len: u32,
}

#[derive(Debug, Clone)]
pub struct Entry<K: Debug + Clone, V: Debug + Clone> {
    key: K,
    value: V,
}

impl<K: Debug + Clone, V: Debug + Clone> Entry<K, V> {
    pub fn get_key(&self) -> &K {
        &self.key
    }

    pub fn get_value(&self) -> &V {
        &self.value
    }
}

#[derive(Debug)]
enum ProbeResult {
    Found(usize), // key exists at this index
    Empty(usize), // key is absent, but we can insert here. Likely that original slot was taken
    Full,         // table is full, need to grow
}

//--------------utils---------------------------
// macro_rules! hash_str {
//     ($val:expr) => {{
//         let s: &str = $val; // complie-time type assertion.
//         fnv1_hash(s) as usize
//     }};
// }
// pub fn fnv1_hash(key: &str) -> u32 {
//     key.bytes()
//         .fold(2166136261u32, |hash, b| (hash ^ (b as u32)) * 16777619)
// }

pub fn fnv1_hash(key: SymbolU32) -> u32 {
    let hash = 2166136261u32;
    (hash ^ (key.to_usize() as u32)) * 16777619u32
}

pub struct Iter<'a> {
    iter: std::iter::Flatten<std::slice::Iter<'a, Option<Entry<SymbolU32, Value>>>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Entry<SymbolU32, Value>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub struct IterMut<'a> {
    iter: std::iter::Flatten<std::slice::IterMut<'a, Option<Entry<SymbolU32, Value>>>>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut Entry<SymbolU32, Value>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl Default for HashTable {
    fn default() -> Self {
        Self::new()
    }
}

impl HashTable {
    pub const fn new() -> Self {
        Self {
            len: 0,
            entries: Vec::new(),
        }
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter {
            iter: self.entries.iter().flatten(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut {
            iter: self.entries.iter_mut().flatten(),
        }
    }

    // checks if key exists.
    fn get_key_index(&self, key: SymbolU32) -> ProbeResult {
        let len = self.entries.len();
        // NOTE: instead of hashing SymbolU32 are already unique.
        // we can use them as hashed ids of the strings interned.
        let start: usize = key.to_usize() % len;
        let mut index = start;
        loop {
            match &self.entries[index] {
                Some(entry) if entry.key == key => return ProbeResult::Found(index), //return &mut self.entries[index],
                None => return ProbeResult::Empty(index),
                _ => index = (index + 1) % len,
            }

            if index == start {
                return ProbeResult::Full;
            }
        }
    }

    // returns true if no previous entry existed. i.e inserted new value.
    pub fn insert(&mut self, key: SymbolU32, v: Value) -> bool {
        let entry = Some(Entry { key, value: v });
        if self.entries.is_empty() {
            self.entries.push(entry);
            self.len += 1;
            return true;
        }

        match self.get_key_index(key) {
            ProbeResult::Empty(index) => {
                self.entries[index] = entry;
                self.len += 1;
                true
            }
            ProbeResult::Found(index) => {
                self.entries[index] = entry;
                false
            }
            ProbeResult::Full => {
                self.entries.push(entry);
                self.len += 1;
                true
            }
        }
    }

    pub fn add_all(&mut self, other: HashTable) {
        self.entries.reserve(other.entries.len());
        // if iterator and into_iter is not implemented.
        // into_iter().flatten() is the correct approach.
        for entry in other.into_iter() {
            self.insert(entry.key, entry.value);
        }
    }

    pub fn get(&self, key: SymbolU32) -> Option<Value> {
        if self.entries.is_empty() {
            return None;
        }

        match self.get_key_index(key) {
            ProbeResult::Found(index) => {
                let entry = self.entries[index].as_ref();
                Some(entry.unwrap().value.clone())
            }
            _ => None,
        }
    }

    /// returns a Some(Value) if the key exists
    /// and none if it doesn't.
    pub fn delete(&mut self, key: SymbolU32) -> Option<Entry<SymbolU32, Value>> {
        if self.entries.is_empty() {
            return None;
        }
        let len = self.entries.len();
        match self.get_key_index(key) {
            ProbeResult::Empty(_) | ProbeResult::Full => None, // there is nothing to remove
            ProbeResult::Found(index) => {
                // std::mem::replace(&mut self.entries[index], None)
                let removed: Option<Entry<SymbolU32, Value>> = self.entries[index].take();
                if removed.is_some() {
                    self.len -= 1;

                    // rehash all entries following the deleted slot.
                    let mut i = (index + 1) % len;
                    while let Some(entry) = self.entries[i].take() {
                        self.insert(entry.key, entry.value);
                        i = (i + 1) % len;
                    }
                }
                removed
            }
        }
    }

    pub fn contains_key(&self, key: SymbolU32) -> bool {
        matches!(self.get_key_index(key), ProbeResult::Found(_))
    }
}

// holds iterator state.
pub struct MyIntoIter {
    iter: std::vec::IntoIter<Option<Entry<SymbolU32, Value>>>,
}

// defines how to advance / consume next item.
impl Iterator for MyIntoIter {
    type Item = Entry<SymbolU32, Value>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(Some(entry)) => return Some(entry),
                Some(None) => continue, // empty slot
                None => return None,
            }
        }
    }
}

// defines how to turn value into an iterator.
impl IntoIterator for HashTable {
    type Item = Entry<SymbolU32, Value>;

    type IntoIter = MyIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        MyIntoIter {
            iter: self.entries.into_iter(),
        }
    }
}
