pub mod interner;

use string_interner::{Symbol, symbol::SymbolU32};

use crate::value::Value;
use std::fmt::Debug;

// #[derive(Debug)]
// pub struct HashTable<K: Eq + Debug + Clone, V: Debug + Clone> {
//     // count: usize, // redundant, Vec already holds this info.
//     // capacity: usize,
//     entries: Vec<Option<Entry<K, V>>>,
// }
// #[derive(Debug)]
// struct Entry<K: Debug + Clone, V: Debug + Clone> {
//     key: K,
//     value: V
//  }

/// K: SymbolU32 is the interned string id, the Value::String(SymbolU32)
/// already carries required information, no need duplicating the interened String again.
#[derive(Debug)]
pub struct HashTable {
    /// Vec<Option<Entry>> is used here for open addressing, i.e (find the next empty spot when keys collide)
    /// Some = occupied,
    /// None = empty slot, terminate probing.
    pub entries: Vec<Option<Entry<SymbolU32, Value>>>,
    len: u32,
}

#[derive(Debug)]
pub struct Entry<K: Debug + Clone, V: Debug + Clone> {
    key: K,
    value: V,
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

impl HashTable {
    pub const fn new() -> Self {
        Self {
            len: 0,
            entries: Vec::new(),
        }
    }

    pub fn iter(&mut self) -> Iter<'_> {
        Iter {
            iter: self.entries.iter().flatten(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut {
            iter: self.entries.iter_mut().flatten(),
        }
    }

    // eeewwwww
    fn find_entry_mut(&mut self, key: SymbolU32) -> Option<&mut Option<Entry<SymbolU32, Value>>> {
        match self.get_key_index(key) {
            Some(index) => Some(&mut self.entries[index]),
            None => None,
        }
    }

    fn find_entry(&self, key: SymbolU32) -> &Option<Entry<SymbolU32, Value>> {
        match self.get_key_index(key) {
            Some(index) => &self.entries[index],
            None => &None,
        }
    }

    // checks if key exists.
    fn get_key_index(&self, key: SymbolU32) -> Option<usize> {
        let len = self.entries.len();
        // NOTE: instead of hashing SymbolU32 are already unique.
        // we can use them as hashed ids of the strings interned.
        let start: usize = key.to_usize() % len;
        let mut index = start;
        loop {
            match &self.entries[index] {
                Some(entry) if entry.key == key => break, //return &mut self.entries[index],
                None => break, //  return &mut self.entries[index], // stop probing
                _ => index = (index + 1) % len,
            }

            // probe sequence.
            if index == start {
                return None;
            }
        }
        Some(index)
    }

    pub fn insert(&mut self, key: SymbolU32, v: Value) -> bool {
        if self.entries.is_empty() {
            self.entries.push(Some(Entry { key: key, value: v }));
            return true;
        }

        match self.find_entry_mut(key) {
            // some entry is found, value has to be replaced.
            Some(option) if option.is_some() => {
                let prev = std::mem::replace(option, Some(Entry { key: key, value: v }));
                if prev.is_none() {
                    self.len += 1;
                    true
                } else {
                    false
                }
            }
            None => {
                self.entries.push(Some(Entry { key: key, value: v }));
                return true;
            },
            // this case should never exist.
            Some(_) => false,
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
        match self.find_entry(key).as_ref() {
            Some(entry) => {
                return Some(entry.value.clone());
            }
            None => return None,
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
            None => None,
            Some(index) => {
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

    pub fn contains_key<T>(_key: T) -> bool {
        todo!()
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
