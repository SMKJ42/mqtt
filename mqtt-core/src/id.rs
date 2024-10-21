#[cfg(feature = "bitpack")]
use core::ops::{BitAnd, BitOrAssign, BitXorAssign, Shr};

/// Creates a persistant storage for packet id's for QoS 2,
/// and iterates through packet Id's for packets of QoS 1.
///
/// When the "bitpack" feature, utilizes each bit as a flag to indicate if the Id is currently
/// in use. This creates come overhead for compute time, but saves on memory.
///
/// Clients are allotted all even numbers to u16::MAX, while Brokers are allocated odd numbers.
///
#[derive(Debug, Clone)]
pub struct IdGenerator {
    last: u16,
    #[cfg(feature = "bitpack")]
    pub id_table: [u8; u16::MAX as usize / 8 + 1],
    #[cfg(not(feature = "bitpack"))]
    pub id_table: [bool; u16::MAX as usize + 1],
}

impl IdGenerator {
    /// registers an Id as available.
    pub fn free_id(&mut self, id: u16) {
        self.unset(id);
    }

    /// Returns the next available Id.
    pub fn next_persistant_id(&mut self) -> Option<u16> {
        self.next_id().and_then(|idx| {
            self.set_id(idx);
            return Some(idx);
        })
    }

    pub fn next_id(&mut self) -> Option<u16> {
        let mut curr_idx = checked_incr(self.last);
        loop {
            // we hit every index, and no Id's are available.
            // println!("curr: {}, last {}", curr_idx, self.last);

            // zero is an invalid id.
            if curr_idx == 0 {
                curr_idx = checked_incr(curr_idx);
                continue;
            }

            if curr_idx == self.last {
                return None;
            }

            if !self.is_set(curr_idx) {
                self.last = curr_idx;
                return Some(curr_idx);
            }
            curr_idx = checked_incr(curr_idx);
        }
    }
}

#[cfg(feature = "bitpack")]
impl IdGenerator {
    pub fn new(_type: IdGenType) -> Self {
        let last = if _type == IdGenType::Client {
            u16::MAX
        } else {
            u16::MAX - 1
        };
        return Self {
            last,
            id_table: [0; u16::MAX as usize / 8 + 1],
        };
    }

    pub fn flush(&mut self) {
        self.id_table = [0; u16::MAX as usize / 8 + 1]
    }

    /// internal use for iterating through the Ids.
    pub fn is_set(&self, idx: u16) -> bool {
        let chunk: u16 = idx / 8;
        let target_bit = 0b1000_0000.shr(idx % 8) as u8;
        self.id_table[chunk as usize].bitand(target_bit) == target_bit
    }

    /// registers an Id as available.
    pub fn unset(&mut self, idx: u16) {
        let chunk: u16 = idx / 8;
        let target_bit: u8 = 0b1000_0000.shr(idx % 8) as u8;
        self.id_table[chunk as usize].bitxor_assign(target_bit);
    }

    /// registers an Id as taken.
    pub fn set_id(&mut self, idx: u16) {
        let chunk: u16 = idx / 8;
        let target_bit = 0b1000_0000.shr(idx % 8) as u8;
        self.id_table[chunk as usize].bitor_assign(target_bit);
    }
}

fn checked_incr(int: u16) -> u16 {
    match int.checked_add(2) {
        Some(idx) => idx,
        None => {
            return int % 2;
        }
    }
}

#[cfg(not(feature = "bitpack"))]
impl IdGenerator {
    pub fn new(_type: IdGenType) -> Self {
        let last = if _type == IdGenType::Client {
            u16::MAX
        } else {
            u16::MAX - 1
        };
        return Self {
            last,
            id_table: [false; u16::MAX as usize + 1],
        };
    }

    pub fn flush(&mut self) {
        self.id_table = [false; u16::MAX as usize + 1]
    }

    /// internal use for iterating through the Ids.
    pub fn is_set(&self, idx: u16) -> bool {
        return self.id_table[idx as usize] == true;
    }

    /// registers an Id as available.
    pub fn unset(&mut self, idx: u16) {
        self.id_table[idx as usize] = false;
    }

    /// registers an Id as taken.
    pub fn set_id(&mut self, idx: u16) {
        self.id_table[idx as usize] = true;
    }
}

/// Clients are allotted all even numbers to u16::MAX, while Brokers are allocated odd numbers.
#[derive(Clone, Copy, PartialEq)]
pub enum IdGenType {
    Client,
    Broker,
}

#[cfg(test)]
mod id_gen {
    use crate::id::IdGenType;

    use super::IdGenerator;

    #[test]
    fn basic() {
        let mut gen = IdGenerator::new(IdGenType::Broker);
        let id = gen.next_persistant_id();
        assert_eq!(id, Some(2));

        let mut gen = IdGenerator::new(IdGenType::Client);
        let id = gen.next_persistant_id();
        assert_eq!(id, Some(1));
    }
    #[test]
    fn filled_then_unset() {
        let mut gen = IdGenerator::new(IdGenType::Broker);
        for _ in 0..=u16::MAX / 2 {
            gen.next_persistant_id();
        }

        assert_eq!(None, gen.next_id());
        for i in 0..=u16::MAX / 2 {
            if i % 2 == 0 {
                gen.free_id(i);
            }
        }

        assert_eq!(gen.next_persistant_id(), Some(2));

        let mut gen = IdGenerator::new(IdGenType::Client);
        for _ in 0..=u16::MAX / 2 {
            gen.next_persistant_id();
        }

        assert_eq!(None, gen.next_id());
        for i in 0..=u16::MAX {
            if i % 2 == 1 {
                gen.free_id(i);
            }
        }

        assert_eq!(gen.next_persistant_id(), Some(1));
    }
}
