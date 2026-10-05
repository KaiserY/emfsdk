// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! TrueType hinting.

mod call_stack;
mod compatible;
mod cow_slice;
mod cvt;
mod definition;
mod engine;
mod error;
mod graphics;
mod instance;
mod math;
mod program;
mod projection;
mod round;
mod storage;
mod value_stack;
mod zone;

use super::Target;

use skrifa::raw::{
  tables::glyf::PointFlags,
  types::{F2Dot14, F26Dot6, GlyphId, Point},
};

pub use instance::HintInstance;

/// Outline data that is passed to the hinter.
pub struct HintOutline<'a> {
  pub glyph_id: GlyphId,
  pub compatible_advance: Option<F26Dot6>,
  pub unscaled: &'a [Point<i32>],
  pub scaled: &'a mut [Point<F26Dot6>],
  pub original_scaled: &'a mut [Point<F26Dot6>],
  pub flags: &'a mut [PointFlags],
  pub contours: &'a [u16],
  pub phantom: &'a mut [Point<F26Dot6>],
  pub bytecode: &'a [u8],
  pub stack: &'a mut [i32],
  pub cvt: &'a mut [i32],
  pub storage: &'a mut [i32],
  pub twilight_scaled: &'a mut [Point<F26Dot6>],
  pub twilight_original_scaled: &'a mut [Point<F26Dot6>],
  pub twilight_flags: &'a mut [PointFlags],
  pub is_composite: bool,
  pub coords: &'a [F2Dot14],
}
