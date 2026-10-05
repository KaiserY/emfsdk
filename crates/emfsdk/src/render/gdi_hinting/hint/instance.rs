// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! Instance state for TrueType hinting.

use super::{
  super::Outlines,
  HintOutline, PointFlags, Target,
  cow_slice::CowSlice,
  definition::{Definition, DefinitionMap, DefinitionState},
  engine::{Engine, EngineInputs},
  error::HintError,
  graphics::RetainedGraphicsState,
  program::{Program, ProgramState},
  value_stack::ValueStack,
  zone::Zone,
};
use skrifa::raw::{
  TableProvider,
  types::{F2Dot14, F26Dot6, Fixed, Point},
};
use std::vec::Vec;

#[derive(Clone, Default)]
pub struct HintInstance {
  functions: Vec<Definition>,
  instructions: Vec<Definition>,
  cvt: Vec<i32>,
  storage: Vec<i32>,
  graphics: RetainedGraphicsState,
  twilight_scaled: Vec<Point<F26Dot6>>,
  twilight_original_scaled: Vec<Point<F26Dot6>>,
  twilight_flags: Vec<PointFlags>,
  axis_count: u16,
  max_stack: usize,
}

impl HintInstance {
  pub fn reconfigure(
    &mut self,
    outlines: &Outlines,
    scale: i32,
    ppem: i32,
    target: Target,
    coords: &[F2Dot14],
  ) -> Result<(), HintError> {
    self.setup(outlines, scale, coords);
    let twilight_contours = [self.twilight_scaled.len() as u16];
    let twilight = Zone::new(
      &[],
      &mut self.twilight_original_scaled,
      &mut self.twilight_scaled,
      &mut self.twilight_flags,
      &twilight_contours,
    );
    let glyph = Zone::default();
    let mut stack_buf = vec![0; self.max_stack];
    let value_stack = ValueStack::new(&mut stack_buf, false);
    let graphics = RetainedGraphicsState::new(scale, ppem, target);
    let mut engine = Engine::new(
      outlines,
      EngineInputs {
        compatible_advance: None,
        program: ProgramState::new(outlines.fpgm, outlines.prep, &[], Program::Font),
        graphics,
        definitions: DefinitionState::new(
          DefinitionMap::Mut(&mut self.functions),
          DefinitionMap::Mut(&mut self.instructions),
        ),
        cvt: CowSlice::new_mut(&mut self.cvt).into(),
        storage: CowSlice::new_mut(&mut self.storage).into(),
        value_stack,
        twilight,
        glyph,
        axis_count: self.axis_count,
        coords,
        is_composite: false,
      },
    );
    // Run the font program (fpgm)
    engine.run_program(Program::Font, false)?;
    // Run the control value program (prep)
    engine.run_program(Program::ControlValue, false)?;
    // Save the retained state from the CV program
    self.graphics = *engine.retained_graphics_state();
    Ok(())
  }

  /// Returns true if we should actually apply hinting.
  ///
  /// Hinting can be completely disabled by the control value program.
  pub fn is_enabled(&self) -> bool {
    // If bit 0 is set, disables hinting entirely
    self.graphics.instruct_control & 1 == 0
  }

  pub fn hint(
    &self,
    outlines: &Outlines,
    outline: &mut HintOutline,
    is_pedantic: bool,
  ) -> Result<(), HintError> {
    // Twilight zone
    let twilight_count = outline.twilight_scaled.len();
    let twilight_contours = [twilight_count as u16];
    outline
      .twilight_original_scaled
      .copy_from_slice(&self.twilight_original_scaled);
    outline
      .twilight_scaled
      .copy_from_slice(&self.twilight_scaled);
    outline.twilight_flags.copy_from_slice(&self.twilight_flags);
    let twilight = Zone::new(
      &[],
      outline.twilight_original_scaled,
      outline.twilight_scaled,
      outline.twilight_flags,
      &twilight_contours,
    );
    // Glyph zone
    let glyph = Zone::new(
      outline.unscaled,
      outline.original_scaled,
      outline.scaled,
      outline.flags,
      outline.contours,
    );
    let value_stack = ValueStack::new(outline.stack, is_pedantic);
    let cvt = CowSlice::new(&self.cvt, outline.cvt).unwrap();
    let storage = CowSlice::new(&self.storage, outline.storage).unwrap();
    let mut engine = Engine::new(
      outlines,
      EngineInputs {
        compatible_advance: outline.compatible_advance,
        program: ProgramState::new(
          outlines.fpgm,
          outlines.prep,
          outline.bytecode,
          Program::Glyph,
        ),
        graphics: self.graphics,
        definitions: DefinitionState::new(
          DefinitionMap::Ref(&self.functions),
          DefinitionMap::Ref(&self.instructions),
        ),
        cvt: cvt.into(),
        storage: storage.into(),
        value_stack,
        twilight,
        glyph,
        axis_count: self.axis_count,
        coords: outline.coords,
        is_composite: outline.is_composite,
      },
    );
    engine
      .run_program(Program::Glyph, is_pedantic)
      .map_err(|mut e| {
        e.glyph_id = Some(outline.glyph_id);
        e
      })?;
    // If we're not running in backward compatibility mode, capture
    // modified phantom points.
    if !engine.backward_compatibility() {
      for (i, p) in (outline.scaled[outline.scaled.len() - 4..])
        .iter()
        .enumerate()
      {
        outline.phantom[i] = *p;
      }
    }
    Ok(())
  }

  /// Captures limits, resizes buffers and scales the CVT.
  fn setup(&mut self, outlines: &Outlines, scale: i32, coords: &[F2Dot14]) {
    let axis_count = outlines
      .gvar
      .as_ref()
      .map(|gvar| gvar.axis_count())
      .unwrap_or_default();
    self.functions.clear();
    self
      .functions
      .resize(outlines.max_function_defs as usize, Definition::default());
    self.instructions.resize(
      outlines.max_instruction_defs as usize,
      Definition::default(),
    );
    self.cvt.clear();
    let cvt = outlines.font.cvt().unwrap_or_default();
    if let Ok(cvar) = outlines.font.cvar() {
      // First accumulate all the deltas in 16.16
      self.cvt.resize(cvt.len(), 0);
      let _ = cvar.deltas(axis_count, coords, &mut self.cvt);
      // Now add the base CVT values
      for (value, base_value) in self.cvt.iter_mut().zip(cvt.iter()) {
        // Deltas are converted from 16.16 to 26.6
        // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttgxvar.c#L3822>
        let delta = Fixed::from_bits(*value).to_f26dot6().to_bits();
        let base_value = base_value.get() as i32 * 64;
        *value = base_value + delta;
      }
    } else {
      // CVT values are converted to 26.6 on load
      // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttpload.c#L350>
      self
        .cvt
        .extend(cvt.iter().map(|value| (value.get() as i32) * 64));
    }
    // More weird scaling. This is due to the fact that CVT values are
    // already in 26.6
    // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttobjs.c#L996>
    let scale = Fixed::from_bits(scale >> 6);
    for value in &mut self.cvt {
      // Windows rounds signed CVT scaling ties toward positive infinity.
      // Fixed's symmetric rounding instead takes negative ties away from zero.
      *value = ((i64::from(*value) * i64::from(scale.to_bits()) + 0x8000) >> 16) as i32;
    }
    self.storage.clear();
    self.storage.resize(outlines.max_storage as usize, 0);
    let max_twilight_points = outlines.max_twilight_points as usize;
    self.twilight_scaled.clear();
    self
      .twilight_scaled
      .resize(max_twilight_points, Default::default());
    self.twilight_original_scaled.clear();
    self
      .twilight_original_scaled
      .resize(max_twilight_points, Default::default());
    self.twilight_flags.clear();
    self
      .twilight_flags
      .resize(max_twilight_points, Default::default());
    self.axis_count = axis_count;
    self.max_stack = outlines.max_stack_elements as usize;
    self.graphics = RetainedGraphicsState::default();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cvt_scaling_matches_native_signed_half_ties() {
    let font = skrifa::FontRef::from_index(
      include_bytes!("../../testdata/cleartype-cvt-rounding.ttf"),
      0,
    )
    .unwrap();
    let outlines = Outlines::new(&font).unwrap();
    let mut instance = HintInstance::default();
    // The owned font encodes each RCVT result into a positive advance.
    // These are actual DirectWrite GDI Natural readbacks at 67 ppem,
    // decoded back to F26.6; no font installation or Windows is needed here.
    instance.setup(
      &outlines,
      (Fixed::from_bits(67 * 64) / Fixed::from_bits(4096)).to_bits(),
      &[],
    );
    assert_eq!(
      instance.cvt,
      [
        -352, -335, -318, -301, -285, -268, -251, -234, -218, -201, -184, -167, -151, -134, -117,
        -100, -84, -67, -50, -33, -17, 0, 17, 34, 50, 67, 84, 101, 117, 134, 151, 168, 184, 201,
        218, 235, 251, 268, 285, 302, 318, 335,
      ]
    );
  }
}
