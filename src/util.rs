use mmap::MemoryMap;
use std::{ops::Range, slice};

/// Trait for ranges containing values.
pub trait RangeContains<Idx: PartialOrd<Idx>> {
  fn contains_(&self, item: Idx) -> bool;
}

impl<Idx: PartialOrd<Idx>> RangeContains<Idx> for Range<Idx> {
  fn contains_(&self, item: Idx) -> bool {
    self.start <= item && self.end > item
  }
}

/// A wrapper for making a memory map compatible with `SlicePool`.
pub struct SliceableMemoryMap(MemoryMap);

impl SliceableMemoryMap {
  pub fn new(map: MemoryMap) -> Self {
    SliceableMemoryMap(map)
  }

  pub fn as_slice(&self) -> &[u8] {
    unsafe { slice::from_raw_parts(self.0.data(), self.0.len()) }
  }

  pub fn as_mut_slice(&mut self) -> &mut [u8] {
    unsafe { slice::from_raw_parts_mut(self.0.data(), self.0.len()) }
  }
}

impl AsRef<[u8]> for SliceableMemoryMap {
  fn as_ref(&self) -> &[u8] {
    self.as_slice()
  }
}

impl AsMut<[u8]> for SliceableMemoryMap {
  fn as_mut(&mut self) -> &mut [u8] {
    self.as_mut_slice()
  }
}

unsafe impl Send for SliceableMemoryMap {}
unsafe impl Sync for SliceableMemoryMap {}
