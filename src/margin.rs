use std::ops::Range;

/// Represents a margin relative to an origin.
pub trait Margin {
  /// Returns the margin as an address range.
  fn as_range(&self, origin: *const ()) -> Range<usize>;
}

impl Margin for Range<isize> {
  fn as_range(&self, origin: *const ()) -> Range<usize> {
    let lower = if self.start < 0 {
      (origin as usize).saturating_sub(self.start.abs() as usize)
    } else {
      (origin as usize).saturating_add(self.start as usize)
    };

    let upper = if self.end < 0 {
      (origin as usize).saturating_sub(self.end.abs() as usize)
    } else {
      (origin as usize).saturating_add(self.end as usize)
    };

    lower..upper
  }
}

impl Margin for usize {
  fn as_range(&self, origin: *const ()) -> Range<usize> {
    let lower = (origin as usize).saturating_sub(*self);
    let upper = (origin as usize).saturating_add(*self);
    lower..upper
  }
}
