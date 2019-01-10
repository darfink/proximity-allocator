#![deny(missing_docs)]
//! A library for allocating memory in proximity.

extern crate mmap_fixed as mmap;
extern crate region;
extern crate slice_pool;
extern crate tap;

pub use self::error::{Error, Result};
pub use self::margin::Margin;
use self::util::RangeContains;
pub use mmap::MapOption;
use mmap::MemoryMap;
use slice_pool::sync::{SliceBox, SlicePool};
use std::ops::{Deref, DerefMut, Range};
use std::sync::{Arc, RwLock, Weak};
use std::{collections::BTreeMap, fmt, iter};
use tap::TapOptionOps;

// TODO: Support small margins/ranges (i.e less than PAGE_SIZE)

mod error;
mod margin;
mod search;
mod util;

/// A thread-safe memory allocator based on proximity.
pub struct ProximityAllocator {
  // TODO: Actually use a range as key
  pools: RwLock<BTreeMap<usize, Weak<SlicePool<u8>>>>,
  options: Vec<MapOption>,
}

impl ProximityAllocator {
  /// Constructs a new allocator with read/write/execute properties.
  pub fn new() -> Self {
    Self::with_options(vec![
      MapOption::MapReadable,
      MapOption::MapWritable,
      MapOption::MapExecutable,
    ])
  }

  /// Constructs a new memory pool with the specified options.
  pub fn with_options(options: Vec<MapOption>) -> Self {
    let pools = RwLock::new(BTreeMap::new());
    ProximityAllocator { pools, options }
  }

  /// Allocates memory regardless of proximity.
  ///
  /// # Panics
  ///
  /// - Panics if the size zero.
  pub fn alloc(&self, size: usize) -> Result<ProximityBox> {
    assert!(size > 0);
    let range = 0..usize::max_value();
    self
      .alloc_with_existing_pool(size, range.clone())
      .or_else(|_| {
        println!("ALLOCING WITH NEW");
        let pool = self.reserve_memory_map(size, None)?;
        self.alloc_with_new_pool(pool, size, range)
      })
  }

  /// Allocates proximity memory within a margin.
  ///
  /// # Panics
  ///
  /// - Panics if the size zero.
  pub fn alloc_with_margin(
    &self,
    size: usize,
    origin: *const (),
    margin: impl Margin,
  ) -> Result<ProximityBox> {
    self.alloc_with_range(size, origin, margin.as_range(origin))
  }

  /// Allocates proximity memory within an address range.
  ///
  /// # Panics
  ///
  /// - Panics if the size zero.
  /// - Panics if the address `range` does not contain `origin`.
  pub fn alloc_with_range(
    &self,
    size: usize,
    origin: *const (),
    range: Range<usize>,
  ) -> Result<ProximityBox> {
    assert!(size > 0);
    assert!(range.contains_(origin as usize));
    self
      .alloc_with_existing_pool(size, range.clone())
      .or_else(|_| {
        let pool = self.reserve_memory_map_within_range(size, origin, range.clone())?;
        self.alloc_with_new_pool(pool, size, range)
      })
  }

  fn alloc_with_existing_pool(&self, size: usize, range: Range<usize>) -> Result<ProximityBox> {
    let mut inactive_pools = Vec::new();
    let allocation = {
      let pools = self.pools.read().expect("poisoned lock");
      let pools_within_range = pools.range(range);
      pools_within_range
        .filter_map(|(key, pool)| {
          let pool = pool.upgrade().tap_none(|| inactive_pools.push(*key))?;
          let data = pool.alloc(size)?; // TODO: Validate range within pool
          Some(ProximityBox { pool, data })
        }).next()
    };

    if !inactive_pools.is_empty() {
      let mut pools = self.pools.write().expect("poisoned lock");
      for key in inactive_pools {
        pools.remove(&key);
      }
    }

    allocation.ok_or(Error::OutOfMemory)
  }

  fn alloc_with_new_pool(
    &self,
    pool: SlicePool<u8>,
    size: usize,
    _: Range<usize>,
  ) -> Result<ProximityBox> {
    // TODO: Validate range within pool
    let data = pool.alloc(size).ok_or(Error::OutOfMemory)?;
    let pool = Arc::new(pool);

    // TODO: THIS!
    let range = pool.as_ptr() as usize/*..pool.len()*/;
    self
      .pools
      .write()
      .expect("poisoned lock")
      .insert(range, Arc::downgrade(&pool));
    Ok(ProximityBox { pool, data })
  }

  /// Reserves a memory map within a range.
  fn reserve_memory_map_within_range(
    &self,
    size: usize,
    origin: *const (),
    range: Range<usize>,
  ) -> Result<SlicePool<u8>> {
    iter::empty()
      // Search for a free region after & before the origin
      .chain(search::free_regions_after(origin, range.clone()))
      // TODO: Useless on macOS
      .chain(search::free_regions_before(origin, range.clone()))
      // Attempt to allocate a pool for each free region
      .filter_map(|result| {
        match result {
          Ok(address) => self.reserve_memory_map(size, Some(address)).ok().map(Ok),
          Err(error) => Some(Err(error)),
        }
      })
      // Return the first successfully allocated pool
      .next()
      // Otherwise no memory in the proximity could be allocated
      .unwrap_or(Err(Error::OutOfMemory))
  }

  /// Reserves a memory map at an optional fixed address.
  fn reserve_memory_map(&self, size: usize, address: Option<*const ()>) -> Result<SlicePool<u8>> {
    let map = if let Some(address) = address {
      let options = self
        .options
        .iter()
        .cloned()
        .chain([MapOption::MapAddr(address as *const _)].iter().cloned())
        .collect::<Vec<_>>();
      MemoryMap::new(size, &options)
    } else {
      MemoryMap::new(size, &self.options)
    }.map_err(|_| Error::OutOfMemory)?;

    Ok(SlicePool::new(util::SliceableMemoryMap::new(map)))
  }
}

/// A slice for allocated proximity memory.
pub struct ProximityBox {
  #[allow(unused)]
  pool: Arc<SlicePool<u8>>,
  data: SliceBox<u8>,
}

impl fmt::Debug for ProximityBox {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self.deref())
  }
}

impl Deref for ProximityBox {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.data.deref()
  }
}

impl DerefMut for ProximityBox {
  fn deref_mut(&mut self) -> &mut [u8] {
    self.data.deref_mut()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const DISTANCE: usize = 0x1_000_000;

  #[test]
  fn test_margin() {
    let allocator = ProximityAllocator::new();
    let slice1 = allocator
      .alloc_with_margin(0x100, test_margin as *const (), DISTANCE)
      .unwrap();
    let distance = (slice1.as_ptr() as isize - test_margin as isize).abs() as usize;
    assert!(distance <= DISTANCE);
  }

  #[test]
  fn test_pool_reuse() {
    let allocator = ProximityAllocator::new();
    assert!(allocator.pools.read().unwrap().is_empty());
    {
      let _slice1 = allocator.alloc(0x100).unwrap();
      assert_eq!(allocator.pools.read().unwrap().len(), 1);

      let _slice2 = allocator.alloc(0x100).unwrap();
      assert_eq!(allocator.pools.read().unwrap().len(), 1);

      let _slice3 = allocator.alloc(region::page::size()).unwrap();
      assert_eq!(allocator.pools.read().unwrap().len(), 2);
    }

    assert_eq!(allocator.pools.read().unwrap().len(), 2);
    let _slice = allocator.alloc(0x100).unwrap();
    assert_eq!(allocator.pools.read().unwrap().len(), 1);
  }
}
