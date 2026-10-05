// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! Miscellaneous instructions.
//!
//! Implements 3 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#miscellaneous-instructions>

use super::{Engine, OpResult};

impl Engine<'_> {
  /// Get information.
  ///
  /// GETINFO[] (0x88)
  ///
  /// Pops: selector: integer
  /// Pushes: result: integer
  ///
  /// GETINFO is used to obtain data about the font scaler version and the
  /// characteristics of the current glyph. The instruction pops a selector
  /// used to determine the type of information desired and pushes a result
  /// onto the stack.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#get-information>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L6689>
  pub(super) fn op_getinfo(&mut self) -> OpResult {
    use getinfo::*;
    let selector = self.value_stack.pop()?;
    let mut result = 0;
    // Interpreter version (selector bit: 0, result bits: 0-7)
    if (selector & VERSION_SELECTOR_BIT) != 0 {
      result = 42;
    }
    // Glyph rotated (selector bit: 1, result bit: 8)
    if (selector & GLYPH_ROTATED_SELECTOR_BIT) != 0 && self.graphics.is_rotated {
      result |= GLYPH_ROTATED_RESULT_BIT;
    }
    // Glyph stretched (selector bit: 2, result bit: 9)
    if (selector & GLYPH_STRETCHED_SELECTOR_BIT) != 0 && self.graphics.is_stretched {
      result |= GLYPH_STRETCHED_RESULT_BIT;
    }
    // Font variations (selector bit: 3, result bit: 10)
    if (selector & FONT_VARIATIONS_SELECTOR_BIT) != 0 && self.axis_count != 0 {
      result |= FONT_VARIATIONS_RESULT_BIT;
    }
    // The following only apply for smooth hinting.
    if self.graphics.target.is_smooth() {
      // Subpixel hinting [cleartype enabled] (selector bit: 6, result bit: 13)
      // (always enabled)
      if (selector & SUBPIXEL_HINTING_SELECTOR_BIT) != 0 {
        result |= SUBPIXEL_HINTING_RESULT_BIT;
      }
      // GDI quality 5 requests compatible widths; quality 6 requests natural widths.
      if selector & 128 != 0 && self.graphics.target.compatible_widths {
        result |= 16384;
      }
      // Vertical LCD subpixels? (selector bit: 8, result bit: 15)
      if (selector & VERTICAL_LCD_SELECTOR_BIT) != 0 && self.graphics.target.is_vertical_lcd() {
        result |= VERTICAL_LCD_RESULT_BIT;
      }
      // Subpixel positioned? (selector bit: 10, result bit: 17)
      // GDI positions glyph origins at whole device pixels.
      if (selector & SUBPIXEL_POSITIONED_SELECTOR_BIT) != 0 && !self.graphics.target.gdi {
        result |= SUBPIXEL_POSITIONED_RESULT_BIT;
      }
      // Symmetrical smoothing (selector bit: 11, result bit: 18)
      // Note: FreeType always enables this but we allow direct control
      // with our own flag.
      // See <https://github.com/googlefonts/fontations/issues/1080>
      if (selector & SYMMETRICAL_SMOOTHING_SELECTOR_BIT) != 0
        && self.graphics.target.symmetric_rendering()
      {
        result |= SYMMETRICAL_SMOOTHING_RESULT_BIT;
      }
      // ClearType hinting and grayscale rendering (selector bit: 12, result bit: 19)
      if (selector & GRAYSCALE_CLEARTYPE_SELECTOR_BIT) != 0
        && self.graphics.target.is_grayscale_cleartype()
      {
        result |= GRAYSCALE_CLEARTYPE_RESULT_BIT;
      }
    }
    self.value_stack.push(result)
  }

  /// Get variation.
  ///
  /// GETVARIATION[] (0x91)
  ///
  /// Pushes: Normalized axes coordinates, one for each axis in the font.
  ///
  /// GETVARIATION is used to obtain the current normalized variation
  /// coordinates for each axis. The coordinate for the first axis, as
  /// defined in the 'fvar' table, is pushed first on the stack, followed
  /// by each consecutive axis until the coordinate for the last axis is
  /// on the stack.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#get-variation>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L6813>
  pub(super) fn op_getvariation(&mut self) -> OpResult {
    // For non-variable fonts, this falls back to IDEF resolution.
    let axis_count = self.axis_count as usize;
    if axis_count != 0 {
      // Make sure we push `axis_count` coords regardless of the value
      // provided by the user.
      for coord in self
        .coords
        .iter()
        .copied()
        .chain(std::iter::repeat(Default::default()))
        .take(axis_count)
      {
        self.value_stack.push(coord.to_bits() as i32)?;
      }
      Ok(())
    } else {
      self.op_unknown(0x91)
    }
  }

  /// Get data.
  ///
  /// GETDATA[] (0x92)
  ///
  /// Pushes: 17
  ///
  /// Undocumented and nobody knows what this does. FreeType just
  /// returns 17 for variable fonts and falls back to IDEF lookup
  /// otherwise.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L6851>
  pub(super) fn op_getdata(&mut self) -> OpResult {
    if self.axis_count != 0 {
      self.value_stack.push(17)
    } else {
      self.op_unknown(0x92)
    }
  }
}

/// Constants for the GETINFO instruction. Extracted here
/// to enable access from tests.
mod getinfo {
  // Interpreter version (selector bit: 0, result bits: 0-7)
  pub const VERSION_SELECTOR_BIT: i32 = 1 << 0;

  // Glyph rotated (selector bit: 1, result bit: 8)
  pub const GLYPH_ROTATED_SELECTOR_BIT: i32 = 1 << 1;
  pub const GLYPH_ROTATED_RESULT_BIT: i32 = 1 << 8;

  // Glyph stretched (selector bit: 2, result bit: 9)
  pub const GLYPH_STRETCHED_SELECTOR_BIT: i32 = 1 << 2;
  pub const GLYPH_STRETCHED_RESULT_BIT: i32 = 1 << 9;

  // Font variations (selector bit: 3, result bit: 10)
  pub const FONT_VARIATIONS_SELECTOR_BIT: i32 = 1 << 3;
  pub const FONT_VARIATIONS_RESULT_BIT: i32 = 1 << 10;

  // Subpixel hinting [cleartype enabled] (selector bit: 6, result bit: 13)
  // (always enabled)
  pub const SUBPIXEL_HINTING_SELECTOR_BIT: i32 = 1 << 6;
  pub const SUBPIXEL_HINTING_RESULT_BIT: i32 = 1 << 13;

  // Vertical LCD subpixels? (selector bit: 8, result bit: 15)
  pub const VERTICAL_LCD_SELECTOR_BIT: i32 = 1 << 8;
  pub const VERTICAL_LCD_RESULT_BIT: i32 = 1 << 15;

  // Subpixel positioned? (selector bit: 10, result bit: 17)
  // (always enabled)
  pub const SUBPIXEL_POSITIONED_SELECTOR_BIT: i32 = 1 << 10;
  pub const SUBPIXEL_POSITIONED_RESULT_BIT: i32 = 1 << 17;

  // Symmetrical smoothing (selector bit: 11, result bit: 18)
  // Note: FreeType always enables this but we deviate when our own
  // preserve linear metrics flag is enabled.
  pub const SYMMETRICAL_SMOOTHING_SELECTOR_BIT: i32 = 1 << 11;
  pub const SYMMETRICAL_SMOOTHING_RESULT_BIT: i32 = 1 << 18;

  // ClearType hinting and grayscale rendering (selector bit: 12, result bit: 19)
  pub const GRAYSCALE_CLEARTYPE_SELECTOR_BIT: i32 = 1 << 12;
  pub const GRAYSCALE_CLEARTYPE_RESULT_BIT: i32 = 1 << 19;
}
