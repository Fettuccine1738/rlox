pub mod string_interner;

use crate::value::Value;
use std::{fmt::Debug, hash::Hash};

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

/// K: String
#[derive(Debug)]
pub struct HashTable {
    /// Vec<Option<Entry>> is used here for open addressing, i.e (find the next empty spot when keys collide)
    /// Some = occupied,
    /// None =empty slot, terminate probing
    pub entries: Vec<Option<Entry<String, Value>>>,
    len: u32,
}

#[derive(Debug)]
pub struct Entry<K: Debug + Clone, V: Debug + Clone> {
    key: K,
    value: V,
}

//--------------utils---------------------------
macro_rules! hash_str {
    ($val:expr) => {{
        let s: &str = $val; // complie-time type assertion.
        fnv1_hash(s) as usize
    }};
}

pub fn fnv1_hash(key: &str) -> u32 {
    key.bytes()
        .fold(2166136261u32, |hash, b| (hash ^ (b as u32)) * 16777619)
}

pub struct Iter<'a> {
    iter: std::iter::Flatten<std::slice::Iter<'a, Option<Entry<String, Value>>>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Entry<String, Value>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
pub struct IterMut<'a> {
    iter: std::iter::Flatten<std::slice::IterMut<'a, Option<Entry<String, Value>>>>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut Entry<String, Value>;

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

    fn find_entry_mut(&mut self, key: &str) -> &mut Option<Entry<String, Value>> {
        let index = self.get_key_index(key);
        &mut self.entries[index]
    }

    fn find_entry(&self, key: &str) -> &Option<Entry<String, Value>> {
        let index = self.get_key_index(key);
        &self.entries[index]
    }

    fn get_key_index(&self, key: &str) -> usize {
        let len = self.entries.len();
        let start: usize = hash_str!(key) % len;
        let mut index = start;
        loop {
            match &self.entries[index] {
                Some(entry) if entry.key == *key => break, //return &mut self.entries[index],
                None => break, //  return &mut self.entries[index], // stop probing
                _ => index = (index + 1) % len,
            }

            // probe sequence.
            if index == start {
                panic!("hashmap is full!.")
            }
        }
        index
    }

    pub fn insert(&mut self, key: String, v: Value) -> bool {
        let entry = self.find_entry_mut(&key);
        let prev = std::mem::replace(entry, Some(Entry { key: key, value: v }));
        if prev.is_none() {
            self.len += 1;
            true
        } else {
            false
        }
    }

    // pub fn get_entries(&mut self) -> Vec<Option<Entry<String, Value>>> {
    //     self.entries
    // }

    pub fn add_all(&mut self, other: HashTable) {
        self.entries.reserve(other.entries.len());
        // if iterator and into_iter is not implemented.
        // into_iter().flatten() is the correct approach.
        for entry in other.into_iter() {
            self.insert(entry.key, entry.value);
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
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
    pub fn delete(&mut self, key: &str) -> Option<Entry<String, Value>> {
        if self.entries.is_empty() {
            return None;
        }
        let len = self.entries.len();
        let index = self.get_key_index(key);
        // std::mem::replace(&mut self.entries[index], None)
        let removed = self.entries[index].take();
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

// holds iterator state.
pub struct MyIntoIter {
    iter: std::vec::IntoIter<Option<Entry<String, Value>>>,
}

// defines how to advance / consume next item.
impl Iterator for MyIntoIter {
    type Item = Entry<String, Value>;

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
    type Item = Entry<String, Value>;

    type IntoIter = MyIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        MyIntoIter {
            iter: self.entries.into_iter(),
        }
    }
}
