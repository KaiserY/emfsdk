// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! Management of function and instruction definitions.

use core::ops::Range;

use super::{error::HintErrorKind, program::Program};

/// Code range and properties for a function or instruction definition.
// Note: this type is designed to support allocation from user memory
// so make sure the fields are all tightly packed and only use integral
// types.
// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttobjs.h#L158>
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(C)]
pub struct Definition {
  start: u32,
  end: u32,
  /// The function number for an FDEF or opcode for an IDEF.
  key: i32,
  _pad: u16,
  program: u8,
  is_active: u8,
}

impl Definition {
  /// Creates a new definition with the given program, code range and
  /// key.
  ///
  /// The key is either a function number or opcode for function and
  /// instruction definitions respectively.
  pub fn new(program: Program, code_range: Range<usize>, key: i32) -> Self {
    Self {
      program: program as u8,
      // Table sizes are specified in u32 so valid ranges will
      // always fit.
      start: code_range.start as u32,
      end: code_range.end as u32,
      key,
      _pad: 0,
      is_active: 1,
    }
  }

  /// Returns the program that contains this definition.
  pub fn program(&self) -> Program {
    match self.program {
      0 => Program::Font,
      1 => Program::ControlValue,
      _ => Program::Glyph,
    }
  }

  /// Returns the function number or opcode.
  /// Returns the byte range of the code for this definition in the source
  /// program.
  pub fn code_range(&self) -> Range<usize> {
    self.start as usize..self.end as usize
  }

  /// Returns true if this definition entry has been defined by a program.
  pub fn is_active(&self) -> bool {
    self.is_active != 0
  }
}

/// Map of function number or opcode to code definitions.
///
/// The `Ref` vs `Mut` distinction exists because these can be modified
/// from the font and control value programs but not from a glyph program.
/// In addition, hinting instance state is immutable once initialized so
/// this captures that in a type safe way.
pub enum DefinitionMap<'a> {
  Ref(&'a [Definition]),
  Mut(&'a mut [Definition]),
}

impl DefinitionMap<'_> {
  /// Attempts to allocate a new definition entry with the given key.
  ///
  /// Overriding a definition is legal, so if an existing active entry
  /// is found with the same key, that one will be returned. Otherwise,
  /// an inactive entry will be chosen.
  pub fn allocate(&mut self, key: i32) -> Result<&mut Definition, HintErrorKind> {
    let Self::Mut(defs) = self else {
      return Err(HintErrorKind::DefinitionInGlyphProgram);
    };
    // First, see if we can use key as an index.
    //
    // For function definitions in well-behaved fonts (that is, where
    // function numbers fall within 0..max_function_defs) this will
    // always work.
    //
    // For instruction definitions, this will likely never work
    // because the number of instruction definitions is usually small
    // (nearly always 0) and the available opcodes are in the higher
    // ranges of u8 space.
    let ix = if defs
      .get(key as usize)
      .filter(|def| !def.is_active() || def.key == key)
      .is_some()
    {
      // If the entry is inactive or the key matches, we're good.
      key as usize
    } else {
      // Otherwise, walk backward looking for an active entry with
      // a matching key. Keep track of the inactive entry with the
      // highest index.
      let mut last_inactive_ix = None;
      for (i, def) in defs.iter().enumerate().rev() {
        if def.is_active() {
          if def.key == key {
            last_inactive_ix = Some(i);
            break;
          }
        } else if last_inactive_ix.is_none() {
          last_inactive_ix = Some(i);
        }
      }
      last_inactive_ix.ok_or(HintErrorKind::TooManyDefinitions)?
    };
    let def = defs.get_mut(ix).ok_or(HintErrorKind::TooManyDefinitions)?;
    *def = Definition::new(Program::Font, 0..0, key);
    Ok(def)
  }

  /// Returns the definition with the given key.
  pub fn get(&self, key: i32) -> Result<&Definition, HintErrorKind> {
    let defs = match self {
      Self::Mut(defs) => *defs,
      Self::Ref(defs) => *defs,
    };
    // Fast path, try to use key as index.
    if let Some(def) = defs.get(key as usize)
      && def.is_active()
      && def.key == key
    {
      return Ok(def);
    }
    // Otherwise, walk backward doing a linear search.
    for def in defs.iter().rev() {
      if def.is_active() && def.key == key {
        return Ok(def);
      }
    }
    Err(HintErrorKind::InvalidDefinition(key as _))
  }

  /// If the map is mutable, resets all definitions to the default
  /// value.
  pub fn reset(&mut self) {
    if let Self::Mut(defs) = self {
      defs.fill(Default::default())
    }
  }
}

/// State containing font defined functions and instructions.
pub struct DefinitionState<'a> {
  pub functions: DefinitionMap<'a>,
  pub instructions: DefinitionMap<'a>,
}

impl<'a> DefinitionState<'a> {
  pub fn new(functions: DefinitionMap<'a>, instructions: DefinitionMap<'a>) -> Self {
    Self {
      functions,
      instructions,
    }
  }
}
