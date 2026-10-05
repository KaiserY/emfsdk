// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! Copy-on-write buffer for CVT and storage area.

/// Backing store for the CVT and storage area.
///
/// The CVT and storage area are initialized in the control value program
/// with values that are relevant to a particular size and hinting
/// configuration. However, some fonts contain code in glyph programs
/// that write to these buffers. Any modifications made in a glyph program
/// should not affect future glyphs and thus should not persist beyond
/// execution of that program. To solve this problem, a copy of the buffer
/// is made on the first write in a glyph program and all changes are
/// discarded on completion.
///
/// For more context, see <https://gitlab.freedesktop.org/freetype/freetype/-/merge_requests/23>
///
/// # Implementation notes
///
/// The current implementation defers the copy but not the allocation. This
/// is to support the guarantee of no heap allocation when operating on user
/// provided memory. Investigation of hinted Noto fonts suggests that writing
/// to CVT/Storage in glyph programs is common for ttfautohinted fonts so the
/// speculative allocation is likely worthwhile.
pub struct CowSlice<'a> {
  data: &'a [i32],
  data_mut: &'a mut [i32],
  /// True if we've initialized the mutable slice
  use_mut: bool,
}

impl<'a> CowSlice<'a> {
  /// Creates a new copy-on-write slice with the given buffers.
  ///
  /// The `data` buffer is expected to contain the initial data and the content
  /// of `data_mut` is ignored unless the [`set`](Self::set) method is called
  /// in which case a copy will be made from `data` to `data_mut` and the
  /// mutable buffer will be used for all further access.
  ///
  /// Returns [`CowSliceSizeMismatchError`] if `data.len() != data_mut.len()`.
  pub fn new(data: &'a [i32], data_mut: &'a mut [i32]) -> Result<Self, CowSliceSizeMismatchError> {
    if data.len() != data_mut.len() {
      return Err(CowSliceSizeMismatchError(data.len(), data_mut.len()));
    }
    Ok(Self {
      data,
      data_mut,
      use_mut: false,
    })
  }

  /// Creates a new copy-on-write slice with the given mutable buffer.
  ///
  /// This avoids an extra copy and allocation in contexts where the data is
  /// already assumed to be mutable (i.e. when executing `fpgm` and `prep`
  /// programs).
  pub fn new_mut(data_mut: &'a mut [i32]) -> Self {
    Self {
      use_mut: true,
      data: &[],
      data_mut,
    }
  }

  /// Returns the value at the given index.
  ///
  /// If mutable data has been initialized, reads from that buffer. Otherwise
  /// reads from the immutable buffer.
  pub fn get(&self, index: usize) -> Option<i32> {
    if self.use_mut {
      self.data_mut.get(index).copied()
    } else {
      self.data.get(index).copied()
    }
  }

  /// Writes a value to the given index.
  ///
  /// If the mutable buffer hasn't been initialized, first performs a full
  /// buffer copy.
  pub fn set(&mut self, index: usize, value: i32) -> Option<()> {
    // Copy from immutable to mutable buffer if we haven't already
    if !self.use_mut {
      self.data_mut.copy_from_slice(self.data);
      self.use_mut = true;
    }
    *self.data_mut.get_mut(index)? = value;
    Some(())
  }

  pub fn len(&self) -> usize {
    if self.use_mut {
      self.data_mut.len()
    } else {
      self.data.len()
    }
  }
}

/// Error returned when the sizes of the immutable and mutable buffers
/// mismatch when constructing a [`CowSlice`].
#[derive(Clone, Debug)]
pub struct CowSliceSizeMismatchError(usize, usize);

impl std::fmt::Display for CowSliceSizeMismatchError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "size mismatch for immutable and mutable buffers: data.len() = {}, data_mut.len() = {}",
      self.0, self.1
    )
  }
}
