// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! Defining and using functions and instructions.
//!
//! Implements 5 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#defining-and-using-functions-and-instructions>

use skrifa::raw::tables::glyf::bytecode::Opcode;

use super::{
  super::{definition::Definition, program::Program},
  Engine, HintErrorKind, OpResult,
};

/// [Functions|Instructions] may not exceed 64K in size.
/// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#function-definition>
/// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#instruction-definition>
const MAX_DEFINITION_SIZE: usize = u16::MAX as usize;

impl Engine<'_> {
  /// Function definition.
  ///
  /// FDEF[] (0x2C)
  ///
  /// Pops: f: function identifier number
  ///
  /// Marks the start of a function definition. The argument f is a number
  /// that uniquely identifies this function. A function definition can
  /// appear only in the Font Program or the CVT program; attempts to invoke
  /// the FDEF instruction within a glyph program will result in an error.
  /// Functions may not exceed 64K in size.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#function-definition>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L3496>
  pub(super) fn op_fdef(&mut self) -> OpResult {
    let f = self.value_stack.pop()?;
    self.do_def(DefKind::Function, f)
  }

  /// End function definition.
  ///
  /// ENDF[] (0x2D)
  ///
  /// Marks the end of a function definition or an instruction definition.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#end-function-definition>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L3578>
  pub(super) fn op_endf(&mut self) -> OpResult {
    self.program.leave()
  }

  /// Call function.
  ///
  /// CALL[] (0x2B)
  ///
  /// Pops: f: function identifier number
  ///
  /// Calls the function identified by the number f.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#call-function>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L3623>
  pub(super) fn op_call(&mut self) -> OpResult {
    let f = self.value_stack.pop()?;
    self.do_call(DefKind::Function, 1, f)
  }

  /// Loop and call function.
  ///
  /// LOOPCALL[] (0x2a)
  ///
  /// Pops: f: function identifier number
  ///       count: number of times to call the function
  ///
  /// Calls the function f, count number of times.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#loop-and-call-function>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L3704>
  pub(super) fn op_loopcall(&mut self) -> OpResult {
    let f = self.value_stack.pop()?;
    let count = self.value_stack.pop()?;
    if count > 0 {
      self.work_budget.doing_loop_call(count as usize)?;
      self.do_call(DefKind::Function, count as u32, f)
    } else {
      Ok(())
    }
  }

  /// Instruction definition.
  ///
  /// IDEF[] (0x89)
  ///
  /// Pops: opcode
  ///
  /// Begins the definition of an instruction. The instruction definition
  /// terminates when at ENDF, which is encountered in the instruction
  /// stream. Subsequent executions of the opcode popped will be directed
  /// to the contents of this instruction definition (IDEF). IDEFs must be
  /// defined in the Font Program or the CVT Program; attempts to invoke the
  /// IDEF instruction within a glyph program will result in an error. An
  /// IDEF affects only undefined opcodes. If the opcode in question is
  /// already defined, the interpreter will ignore the IDEF. This is to be
  /// used as a patching mechanism for future instructions. Instructions
  /// may not exceed 64K in size.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#instruction-definition>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L3788>
  pub(super) fn op_idef(&mut self) -> OpResult {
    let opcode = self.value_stack.pop()?;
    self.do_def(DefKind::Instruction, opcode)
  }

  /// Catch all for unhandled opcodes which will attempt to dispatch to a
  /// user defined instruction.
  pub(super) fn op_unknown(&mut self, opcode: u8) -> OpResult {
    self.do_call(DefKind::Instruction, 1, opcode as i32)
  }

  /// Common code for FDEF and IDEF.
  fn do_def(&mut self, kind: DefKind, key: i32) -> OpResult {
    if self.program.initial == Program::Glyph {
      return Err(HintErrorKind::DefinitionInGlyphProgram);
    }
    let defs = match kind {
      DefKind::Function => &mut self.definitions.functions,
      DefKind::Instruction => &mut self.definitions.instructions,
    };
    let def = defs.allocate(key)?;
    let start = self.program.decoder.pc;
    while let Some(ins) = self.program.decoder.decode() {
      let ins = ins?;
      match ins.opcode {
        Opcode::FDEF | Opcode::IDEF => return Err(HintErrorKind::NestedDefinition),
        Opcode::ENDF => {
          let range = start..ins.pc + 1;
          if self.graphics.is_pedantic && range.len() > MAX_DEFINITION_SIZE {
            *def = Default::default();
            return Err(HintErrorKind::DefinitionTooLarge);
          }
          *def = Definition::new(self.program.current, range, key);
          return Ok(());
        }
        _ => {}
      }
    }
    Err(HintErrorKind::UnexpectedEndOfBytecode)
  }

  /// Common code for CALL, LOOPCALL and unknown opcode handling.
  fn do_call(&mut self, kind: DefKind, count: u32, key: i32) -> OpResult {
    if count == 0 {
      return Ok(());
    }
    let def = match kind {
      DefKind::Function => self.definitions.functions.get(key),
      DefKind::Instruction => match self.definitions.instructions.get(key) {
        // Remap an invalid definition error to unhandled opcode
        Err(HintErrorKind::InvalidDefinition(opcode)) => Err(HintErrorKind::UnhandledOpcode(
          Opcode::from_byte(opcode as u8),
        )),
        result => result,
      },
    };
    self.program.enter(*def?, count)
  }
}

enum DefKind {
  Function,
  Instruction,
}
