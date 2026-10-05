// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! TrueType bytecode interpreter.

mod arith;
mod control_flow;
mod cvt;
mod data;
mod definition;
mod delta;
mod dispatch;
mod graphics;
mod logical;
mod misc;
mod outline;
mod round;
mod stack;
mod storage;

use skrifa::raw::{
  tables::glyf::bytecode::Instruction,
  types::{F2Dot14, F26Dot6, Point},
};

use super::{
  super::Outlines,
  cvt::Cvt,
  definition::DefinitionState,
  error::{HintError, HintErrorKind},
  graphics::{GraphicsState, RetainedGraphicsState},
  math,
  program::ProgramState,
  storage::Storage,
  value_stack::ValueStack,
  zone::Zone,
};

/// Maximum number of instructions we will execute in `Engine::run()`. This
/// is used to ensure termination of a hinting program.
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/include/freetype/config/ftoption.h#L744>
const MAX_RUN_INSTRUCTIONS: usize = 1_000_000;

pub type OpResult = Result<(), HintErrorKind>;

/// TrueType bytecode interpreter.
pub struct Engine<'a> {
  program: ProgramState<'a>,
  graphics: GraphicsState<'a>,
  definitions: DefinitionState<'a>,
  cvt: Cvt<'a>,
  storage: Storage<'a>,
  value_stack: ValueStack<'a>,
  work_budget: WorkBudget,
  axis_count: u16,
  coords: &'a [F2Dot14],
}

pub struct EngineInputs<'a> {
  pub compatible_advance: Option<F26Dot6>,
  pub program: ProgramState<'a>,
  pub graphics: RetainedGraphicsState,
  pub definitions: DefinitionState<'a>,
  pub cvt: Cvt<'a>,
  pub storage: Storage<'a>,
  pub value_stack: ValueStack<'a>,
  pub twilight: Zone<'a>,
  pub glyph: Zone<'a>,
  pub axis_count: u16,
  pub coords: &'a [F2Dot14],
  pub is_composite: bool,
}

impl<'a> Engine<'a> {
  pub fn new(outlines: &Outlines, inputs: EngineInputs<'a>) -> Self {
    let EngineInputs {
      compatible_advance,
      program,
      graphics,
      definitions,
      cvt,
      storage,
      value_stack,
      twilight,
      glyph,
      axis_count,
      coords,
      is_composite,
    } = inputs;
    let point_count = if glyph.points.is_empty() {
      None
    } else {
      Some(glyph.points.len())
    };
    let compatible_width =
      super::compatible::CompatibleWidth::new(glyph.original, compatible_advance);
    let graphics = GraphicsState {
      compatible_width,
      retained: graphics,
      zones: [twilight, glyph],
      is_composite,
      ..Default::default()
    };
    Self {
      program,
      graphics,
      definitions,
      cvt,
      storage,
      value_stack,
      work_budget: WorkBudget::new(outlines, point_count),
      axis_count,
      coords,
    }
  }

  pub fn backward_compatibility(&self) -> bool {
    self.graphics.backward_compatibility
  }

  pub fn retained_graphics_state(&self) -> &RetainedGraphicsState {
    &self.graphics.retained
  }
}

/// Tracks budgets for control flow to limit execution time.
struct WorkBudget {
  /// Maximum number of times we can do backward jumps or
  /// loop calls.
  loop_limit: usize,
  /// Current number of backward jumps executed.
  backward_jumps: usize,
  /// Current number of loop call iterations executed.
  loop_calls: usize,
  /// Counts number of instructions skipped when handling if/else conditions.
  skipped: usize,
}

impl WorkBudget {
  fn new(outlines: &Outlines, point_count: Option<usize>) -> Self {
    let cvt_len = outlines.cvt_len as usize;
    // Compute limits for loop calls and backward jumps.
    // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L6955>
    let loop_limit = if let Some(point_count) = point_count {
      (point_count * 10).max(50) + (cvt_len / 10).max(50)
    } else {
      300 + 22 * cvt_len
    };
    // FreeType has two variables for neg_jump_counter_max and
    // loopcall_counter_max but sets them to the same value so
    // we'll just use a single limit.
    Self {
      loop_limit,
      backward_jumps: 0,
      loop_calls: 0,
      skipped: 0,
    }
  }

  fn reset(&mut self) {
    self.backward_jumps = 0;
    self.loop_calls = 0;
    self.skipped = 0;
  }

  fn doing_backward_jump(&mut self) -> Result<(), HintErrorKind> {
    self.backward_jumps += 1;
    if self.backward_jumps > self.loop_limit {
      Err(HintErrorKind::ExceededExecutionBudget)
    } else {
      Ok(())
    }
  }

  fn doing_loop_call(&mut self, count: usize) -> Result<(), HintErrorKind> {
    self.loop_calls += count;
    if self.loop_calls > self.loop_limit {
      Err(HintErrorKind::ExceededExecutionBudget)
    } else {
      Ok(())
    }
  }

  fn skipping_instruction(&mut self) -> Result<(), HintErrorKind> {
    self.skipped += 1;
    if self.skipped > MAX_RUN_INSTRUCTIONS {
      Err(HintErrorKind::ExceededExecutionBudget)
    } else {
      Ok(())
    }
  }
}
