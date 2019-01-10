use super::{error::*, util::RangeContains};
use region;
use std::ops::Range;

/// Returns an iterator for free after the specified address.
pub fn free_regions_after(
  origin: *const (),
  range: Range<usize>,
) -> impl Iterator<Item = Result<*const ()>> {
  FreeRegionIter::new(origin, range, SearchDirection::After)
}

/// Returns an iterator for free before the specified address.
pub fn free_regions_before(
  origin: *const (),
  range: Range<usize>,
) -> impl Iterator<Item = Result<*const ()>> {
  FreeRegionIter::new(origin, range, SearchDirection::Before)
}

/// Direction for the region search.
enum SearchDirection {
  Before,
  After,
}

/// An iterator searching for free regions.
struct FreeRegionIter {
  range: Range<usize>,
  search: SearchDirection,
  current: usize,
}

impl FreeRegionIter {
  /// Creates a new iterator for free regions.
  fn new(origin: *const (), range: Range<usize>, search: SearchDirection) -> Self {
    FreeRegionIter {
      range,
      current: origin as usize,
      search,
    }
  }
}

impl Iterator for FreeRegionIter {
  type Item = Result<*const ()>;

  /// Returns the closest free region for the current address.
  fn next(&mut self) -> Option<Self::Item> {
    let page_size = region::page::size();
    let memory = 1..usize::max_value();

    while memory.contains_(self.current) && self.range.contains_(self.current) {
      match region::query(self.current as *const _) {
        Ok(region) => {
          self.current = match self.search {
            SearchDirection::Before => region.lower().saturating_sub(page_size),
            SearchDirection::After => region.upper(),
          }
        }
        Err(error) => {
          // Check whether the region is free, otherwise return the error
          let result = Some(match error {
            region::Error::FreeMemory => Ok(self.current as *const _),
            inner => Err(Error::RegionFailure(inner)),
          });

          // Adjust the offset for repeated calls.
          self.current = match self.search {
            SearchDirection::Before => self.current.saturating_sub(page_size),
            SearchDirection::After => self.current.saturating_add(page_size),
          };

          return result;
        }
      }
    }

    None
  }
}
