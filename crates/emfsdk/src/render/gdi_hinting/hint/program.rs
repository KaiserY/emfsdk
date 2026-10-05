// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! TrueType program management.

use skrifa::raw::tables::glyf::bytecode::Decoder;

use super::{
  call_stack::{CallRecord, CallStack},
  definition::Definition,
  error::HintErrorKind,
};

/// Describes the source for a piece of bytecode.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(u8)]
pub enum Program {
  /// Program that initializes the function and instruction tables. Stored
  /// in the `fpgm` table.
  #[default]
  Font = 0,
  /// Program that initializes CVT and storage based on font size and other
  /// parameters. Stored in the `prep` table.
  ControlValue = 1,
  /// Glyph specified program. Stored per-glyph in the `glyf` table.
  Glyph = 2,
}

/// State for managing active programs and decoding instructions.
pub struct ProgramState<'a> {
  /// Bytecode for each of the three program types, indexed by `Program`.
  pub bytecode: [&'a [u8]; 3],
  /// The initial program when execution begins.
  pub initial: Program,
  /// The currently active program.
  pub current: Program,
  /// Instruction decoder for the currently active program.
  pub decoder: Decoder<'a>,
  /// Tracks nested function and instruction invocations.
  pub call_stack: CallStack,
}

impl<'a> ProgramState<'a> {
  pub fn new(
    font_code: &'a [u8],
    cv_code: &'a [u8],
    glyph_code: &'a [u8],
    initial_program: Program,
  ) -> Self {
    let bytecode = [font_code, cv_code, glyph_code];
    Self {
      bytecode,
      initial: initial_program,
      current: initial_program,
      decoder: Decoder::new(bytecode[initial_program as usize], 0),
      call_stack: CallStack::default(),
    }
  }

  /// Resets the state for execution of the given program.
  pub fn reset(&mut self, program: Program) {
    self.initial = program;
    self.current = program;
    self.decoder = Decoder::new(self.bytecode[program as usize], 0);
    self.call_stack.clear();
  }

  /// Jumps to the code in the given definition and sets it up for
  /// execution `count` times.
  pub fn enter(&mut self, definition: Definition, count: u32) -> Result<(), HintErrorKind> {
    let program = definition.program();
    let pc = definition.code_range().start;
    let bytecode = self.bytecode[program as usize];
    self.call_stack.push(CallRecord {
      caller_program: self.current,
      return_pc: self.decoder.pc,
      current_count: count,
      definition,
    })?;
    self.current = program;
    self.decoder = Decoder::new(bytecode, pc);
    Ok(())
  }

  /// Leaves the code from the definition on the top of the stack.
  ///
  /// If the top call record has a loop count greater than 1, restarts
  /// execution from the beginning of the definition. Otherwise, resumes
  /// execution at the previously active definition.
  pub fn leave(&mut self) -> Result<(), HintErrorKind> {
    let mut record = self.call_stack.pop()?;
    if record.current_count > 1 {
      // This is a loop call with some iterations remaining.
      record.current_count -= 1;
      self.decoder.pc = record.definition.code_range().start;
      self.call_stack.push(record)?;
    } else {
      self.current = record.caller_program;
      // Reset the decoder to the calling program and program counter.
      self.decoder.bytecode = self.bytecode[record.caller_program as usize];
      self.decoder.pc = record.return_pc;
    }
    Ok(())
  }
}
