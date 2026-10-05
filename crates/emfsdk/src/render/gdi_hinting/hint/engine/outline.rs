// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! Managing outlines.
//!
//! Implements 87 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#managing-outlines>

use super::{
  super::{
    graphics::CoordAxis,
    program::Program,
    zone::{PointDisplacement, ZonePointer},
  },
  Engine, F26Dot6, HintErrorKind, OpResult, math,
};

// TypeMan's guarded inline-delta helper occurs in Tahoma, Arial, Verdana and
// Segoe UI under different function numbers. The unguarded helper has the
// same SHPIX body but does not receive the ClearType DiagEndCtrl exception.
// See https://learn.microsoft.com/en-us/typography/cleartype/truetypecleartype
const DIAG_END_CTRL_INLINE_DELTA: &[u8] = &[
  0xb0, 0x02, 0x43, 0x54, 0x58, 0x4b, 0x53, 0x23, 0x4b, 0x51, 0x5a, 0x58, 0x38, 0x1b, 0x21, 0x21,
  0x59, 0x1b, 0x21, 0x21, 0x21, 0x21, 0x59, 0x2d,
];

impl Engine<'_> {
  /// Flip point.
  ///
  /// FLIPPT[] (0x80)
  ///
  /// Pops: p: point number (uint32)
  ///
  /// Uses the loop counter.
  ///
  /// Flips points that are off the curve so that they are on the curve and
  /// points that are on the curve so that they are off the curve. The point
  /// is not marked as touched. The result of a FLIPPT instruction is that
  /// the contour describing part of a glyph outline is redefined.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#flip-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5002>
  pub(super) fn op_flippt(&mut self) -> OpResult {
    let count = self.graphics.loop_counter as usize;
    self.graphics.loop_counter = 1;
    // In backward compatibility mode, don't flip points after IUP has
    // been done.
    if self.graphics.backward_compatibility && self.graphics.did_iup_x && self.graphics.did_iup_y {
      for _ in 0..count {
        self.value_stack.pop()?;
      }
      return Ok(());
    }
    let zone = self.graphics.zone_mut(ZonePointer::Glyph);
    for _ in 0..count {
      let p = self.value_stack.pop_usize()?;
      zone.flip_on_curve(p)?;
    }
    Ok(())
  }

  /// Flip range on.
  ///
  /// FLIPRGON[] (0x81)
  ///
  /// Pops: highpoint: highest point number in range of points to be flipped (uint32)
  ///       lowpoint: lowest point number in range of points to be flipped (uint32)
  ///
  /// Flips a range of points beginning with lowpoint and ending with highpoint so that
  /// any off the curve points become on the curve points. The points are not marked as
  /// touched.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#flip-range-on>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5056>
  pub(super) fn op_fliprgon(&mut self) -> OpResult {
    self.set_on_curve_for_range(true)
  }

  /// Flip range off.
  ///
  /// FLIPRGOFF[] (0x82)
  ///
  /// Pops: highpoint: highest point number in range of points to be flipped (uint32)
  ///       lowpoint: lowest point number in range of points to be flipped (uint32)
  ///
  /// Flips a range of points beginning with lowpoint and ending with
  /// highpoint so that any on the curve points become off the curve points.
  /// The points are not marked as touched.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#flip-range-off>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5094>
  pub(super) fn op_fliprgoff(&mut self) -> OpResult {
    self.set_on_curve_for_range(false)
  }

  /// Shift point by the last point.
  ///
  /// SHP\[a\] (0x32 - 0x33)
  ///
  /// a: 0: uses rp2 in the zone pointed to by zp1
  ///    1: uses rp1 in the zone pointed to by zp0
  ///
  /// Pops: p: point to be shifted
  ///
  /// Uses the loop counter.
  ///
  /// Shift point p by the same amount that the reference point has been
  /// shifted. Point p is shifted along the freedom_vector so that the
  /// distance between the new position of point p and the current position
  /// of point p is the same as the distance between the current position
  /// of the reference point and the original position of the reference point.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#shift-point-by-the-last-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5211>
  pub(super) fn op_shp(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let PointDisplacement {
      dx,
      dy,
      zone,
      point_ix,
    } = gs.point_displacement(opcode)?;
    let count = gs.loop_counter;
    gs.loop_counter = 1;
    for _ in 0..count {
      let p = self.value_stack.pop_usize()?;
      gs.move_zp2_point(p, dx, dy, true)?;
      gs.link_width(gs.zp2, p, zone, point_ix);
    }
    Ok(())
  }

  /// Shift contour by the last point.
  ///
  /// SHC\[a\] (0x34 - 0x35)
  ///
  /// a: 0: uses rp2 in the zone pointed to by zp1
  ///    1: uses rp1 in the zone pointed to by zp0
  ///
  /// Pops: c: contour to be shifted
  ///
  /// Shifts every point on contour c by the same amount that the reference
  /// point has been shifted. Each point is shifted along the freedom_vector
  /// so that the distance between the new position of the point and the old
  /// position of that point is the same as the distance between the current
  /// position of the reference point and the original position of the
  /// reference point. The distance is measured along the projection_vector.
  /// If the reference point is one of the points defining the contour, the
  /// reference point is not moved by this instruction.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#shift-contour-by-the-last-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5266>
  pub(super) fn op_shc(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let contour_ix = self.value_stack.pop_usize()?;
    if !gs.is_pedantic && contour_ix >= gs.zp2().contours.len() {
      return Ok(());
    }
    let point_disp = gs.point_displacement(opcode)?;
    let start = if contour_ix != 0 {
      gs.zp2().contour(contour_ix - 1)? as usize + 1
    } else {
      0
    };
    let end = if gs.zp2.is_twilight() {
      gs.zp2().points.len()
    } else {
      gs.zp2().contour(contour_ix)? as usize + 1
    };
    for i in start..end {
      if point_disp.zone != gs.zp2 || point_disp.point_ix != i {
        gs.move_zp2_point(i, point_disp.dx, point_disp.dy, true)?;
        gs.link_width(gs.zp2, i, point_disp.zone, point_disp.point_ix);
      }
    }
    Ok(())
  }

  /// Shift zone by the last point.
  ///
  /// SHZ\[a\] (0x36 - 0x37)
  ///
  /// a: 0: uses rp2 in the zone pointed to by zp1
  ///    1: uses rp1 in the zone pointed to by zp0
  ///
  /// Pops: e: zone to be shifted
  ///
  /// Shift the points in the specified zone (Z1 or Z0) by the same amount
  /// that the reference point has been shifted. The points in the zone are
  /// shifted along the freedom_vector so that the distance between the new
  /// position of the shifted points and their old position is the same as
  /// the distance between the current position of the reference point and
  /// the original position of the reference point.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#shift-zone-by-the-last-pt>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5318>
  pub(super) fn op_shz(&mut self, opcode: u8) -> OpResult {
    let _e = ZonePointer::try_from(self.value_stack.pop()?)?;
    let gs = &mut self.graphics;
    let point_disp = gs.point_displacement(opcode)?;
    let end = if gs.zp2.is_twilight() {
      gs.zp2().points.len()
    } else if !gs.zp2().contours.is_empty() {
      *gs
        .zp2()
        .contours
        .last()
        .ok_or(HintErrorKind::InvalidContourIndex(0))? as usize
        + 1
    } else {
      0
    };
    for i in 0..end {
      if point_disp.zone != gs.zp2 || i != point_disp.point_ix {
        gs.move_zp2_point(i, point_disp.dx, point_disp.dy, false)?;
      }
    }
    Ok(())
  }

  /// Shift point by a pixel amount.
  ///
  /// SHPIX (0x38)
  ///
  /// Pops: amount: magnitude of the shift (F26Dot6)
  ///       p1, p2,.. pn: points to be shifted
  ///
  /// Uses the loop counter.
  ///
  /// Shifts the points specified by the amount stated. When the loop
  /// variable is used, the amount to be shifted is put onto the stack
  /// only once. That is, if loop = 3, then the contents of the top of
  /// the stack should be point p1, point p2, point p3, amount. The value
  /// amount is expressed in sixty-fourths of a pixel.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#shift-point-by-a-pixel-amount>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5366>
  pub(super) fn op_shpix(&mut self) -> OpResult {
    let preserve_inline_delta = self.graphics.target.gdi
      && self.graphics.freedom_vector.x != 0
      && self.program.call_stack.peek().is_some_and(|record| {
        let definition = record.definition;
        definition.program() == Program::Font
          && self.program.bytecode[Program::Font as usize].get(definition.code_range())
            == Some(DIAG_END_CTRL_INLINE_DELTA)
      });
    let gs = &mut self.graphics;
    let in_twilight = gs.zp0.is_twilight() || gs.zp1.is_twilight() || gs.zp2.is_twilight();
    let amount = self.value_stack.pop()?;
    let dx = F26Dot6::from_bits(math::mul14(amount, gs.freedom_vector.x));
    let dy = F26Dot6::from_bits(math::mul14(amount, gs.freedom_vector.y));
    let count = gs.loop_counter;
    gs.loop_counter = 1;
    let did_iup = gs.did_iup_x && gs.did_iup_y;
    for _ in 0..count {
      let p = self.value_stack.pop_usize()?;
      if gs.backward_compatibility && !preserve_inline_delta {
        if in_twilight
          || (!did_iup
            && ((gs.is_composite && gs.freedom_vector.y != 0)
              || gs.zp2().is_touched(p, CoordAxis::Y)?))
        {
          gs.move_zp2_point(p, dx, dy, true)?;
        }
      } else {
        gs.move_zp2_point(p, dx, dy, true)?;
      }
    }
    Ok(())
  }

  /// Move stack indirect relative point.
  ///
  /// MSIRP\[a\] (0x3A - 0x3B)
  ///
  /// a: 0: do not set rp0 to p
  ///    1: set rp0 to p
  ///
  /// Pops: d: distance (F26Dot6)
  ///       p: point number
  ///
  /// Makes the distance between a point p and rp0 equal to the value
  /// specified on the stack. The distance on the stack is in fractional
  /// pixels (F26Dot6). An MSIRP has the same effect as a MIRP instruction
  /// except that it takes its value from the stack rather than the Control
  /// Value Table. As a result, the cut_in does not affect the results of a
  /// MSIRP. Additionally, MSIRP is unaffected by the round_state.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#move-stack-indirect-relative-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5439>
  pub(super) fn op_msirp(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let mut distance = self.value_stack.pop_f26dot6()?;
    let point_ix = self.value_stack.pop_usize()?;
    if !gs.is_pedantic && !gs.in_bounds([(gs.zp1, point_ix), (gs.zp0, gs.rp0)]) {
      return Ok(());
    }
    if gs.zp1.is_twilight() {
      *gs.zp1_mut().point_mut(point_ix)? = gs.zp0().original(gs.rp0)?;
      gs.move_original(gs.zp1, point_ix, distance)?;
      *gs.zp1_mut().point_mut(point_ix)? = gs.zp1().original(point_ix)?;
    }
    if gs.target.gdi && gs.backward_compatibility {
      let original = gs.dual_project(gs.zp1().original(point_ix)?, gs.zp0().original(gs.rp0)?);
      if original != F26Dot6::ZERO && (distance - original).abs() > gs.gdi_cutin() {
        distance = original;
      }
    }
    let d = gs.project(gs.zp1().point(point_ix)?, gs.zp0().point(gs.rp0)?);
    gs.move_point(gs.zp1, point_ix, distance.wrapping_sub(d))?;
    gs.link_width(gs.zp1, point_ix, gs.zp0, gs.rp0);
    gs.stem_width(point_ix, true);
    gs.rp1 = gs.rp0;
    gs.rp2 = point_ix;
    if (opcode & 1) != 0 {
      gs.rp0 = point_ix;
    }
    Ok(())
  }

  /// Move direct absolute point.
  ///
  /// MDAP\[a\] (0x2E - 0x2F)
  ///
  /// a: 0: do not round the value
  ///    1: round the value
  ///
  /// Pops: p: point number
  ///
  /// Sets the reference points rp0 and rp1 equal to point p. If a=1, this
  /// instruction rounds point p to the grid point specified by the state
  /// variable round_state. If a=0, it simply marks the point as touched in
  /// the direction(s) specified by the current freedom_vector. This command
  /// is often used to set points in the twilight zone.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#move-direct-absolute-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5487>
  pub(super) fn op_mdap(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let p = self.value_stack.pop_usize()?;
    if !gs.is_pedantic && !gs.in_bounds([(gs.zp0, p)]) {
      gs.rp0 = p;
      gs.rp1 = p;
      return Ok(());
    }
    let distance = if (opcode & 1) != 0 {
      let cur_dist = gs.project(gs.zp0().point(p)?, Default::default());
      gs.round(cur_dist) - cur_dist
    } else {
      F26Dot6::ZERO
    };
    gs.move_point(gs.zp0, p, distance)?;
    gs.rp0 = p;
    gs.rp1 = p;
    Ok(())
  }

  /// Move indirect absolute point.
  ///
  /// MIAP\[a\] (0x3E - 0x3F)
  ///
  /// a: 0: do not round the distance and don't use control value cutin
  ///    1: round the distance and use control value cutin
  ///
  /// Pops: n: CVT entry number
  ///       p: point number
  ///
  /// Moves point p to the absolute coordinate position specified by the nth
  /// Control Value Table entry. The coordinate is measured along the current
  /// projection_vector. If a=1, the position will be rounded as specified by
  /// round_state. If a=1, and if the device space difference between the CVT
  /// value and the original position is greater than the
  /// control_value_cut_in, then the original position will be rounded
  /// (instead of the CVT value.)
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#move-indirect-absolute-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5526>
  pub(super) fn op_miap(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let cvt_entry = self.value_stack.pop_usize()?;
    let point_ix = self.value_stack.pop_usize()?;
    let mut distance = self.cvt.get(cvt_entry)?;
    if gs.zp0.is_twilight() {
      // Special behavior for twilight zone.
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5548>
      let fv = gs.freedom_vector;
      let z = gs.zp0_mut();
      let original_point = z.original_mut(point_ix)?;
      original_point.x = F26Dot6::from_bits(math::mul14(distance.to_bits(), fv.x));
      original_point.y = F26Dot6::from_bits(math::mul14(distance.to_bits(), fv.y));
      *z.point_mut(point_ix)? = *original_point;
    }
    let original_distance = gs.project(gs.zp0().point(point_ix)?, Default::default());
    if (opcode & 1) != 0 {
      let delta = (distance.wrapping_sub(original_distance)).abs();
      if delta > gs.gdi_cutin() {
        distance = original_distance;
      }
      distance = gs.round(distance);
    }
    gs.move_point(gs.zp0, point_ix, distance.wrapping_sub(original_distance))?;
    gs.rp0 = point_ix;
    gs.rp1 = point_ix;
    Ok(())
  }

  /// Move direct relative point.
  ///
  /// MDRP\[abcde\] (0xC0 - 0xDF)
  ///
  /// a: 0: do not set rp0 to point p after move
  ///    1: do set rp0 to point p after move
  /// b: 0: do not keep distance greater than or equal to minimum_distance
  ///    1: keep distance greater than or equal to minimum_distance
  /// c: 0: do not round distance
  ///    1: round the distance
  /// de: distance type for engine characteristic compensation
  ///
  /// Pops: p: point number
  ///       
  /// MDRP moves point p along the freedom_vector so that the distance from
  /// its new position to the current position of rp0 is the same as the
  /// distance between the two points in the original uninstructed outline,
  /// and then adjusts it to be consistent with the Boolean settings. Note
  /// that it is only the original positions of rp0 and point p and the
  /// current position of rp0 that determine the new position of point p
  /// along the freedom_vector.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#move-direct-relative-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5610>
  pub(super) fn op_mdrp(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let p = self.value_stack.pop_usize()?;
    if !gs.is_pedantic && !gs.in_bounds([(gs.zp1, p), (gs.zp0, gs.rp0)]) {
      gs.rp1 = gs.rp0;
      gs.rp2 = p;
      if (opcode & 16) != 0 {
        gs.rp0 = p;
      }
      return Ok(());
    }
    let mut original_distance = if gs.zp0.is_twilight() || gs.zp1.is_twilight() {
      gs.dual_project(gs.zp1().original(p)?, gs.zp0().original(gs.rp0)?)
    } else {
      let v1 = gs.zp1().unscaled(p);
      let v2 = gs.zp0().unscaled(gs.rp0);
      let dist = gs.dual_project_unscaled(v1, v2);
      F26Dot6::from_bits(math::mul(dist, gs.unscaled_to_pixels()))
    };
    let cutin = gs.single_width_cutin;
    let value = gs.single_width;
    if cutin > F26Dot6::ZERO
      && original_distance < value + cutin
      && original_distance > value - cutin
    {
      original_distance = if original_distance >= F26Dot6::ZERO {
        value
      } else {
        -value
      };
    }
    // round flag
    let mut distance = if (opcode & 4) != 0 {
      gs.round(original_distance)
    } else {
      original_distance
    };
    // minimum distance flag
    if (opcode & 8) != 0 {
      let min_distance = gs.gdi_minimum();
      if original_distance >= F26Dot6::ZERO {
        if distance < min_distance {
          distance = min_distance;
        }
      } else if distance > -min_distance {
        distance = -min_distance;
      }
    }
    original_distance = gs.project(gs.zp1().point(p)?, gs.zp0().point(gs.rp0)?);
    gs.move_point(gs.zp1, p, distance.wrapping_sub(original_distance))?;
    gs.link_width(gs.zp1, p, gs.zp0, gs.rp0);
    gs.rp1 = gs.rp0;
    gs.rp2 = p;
    if (opcode & 16) != 0 {
      gs.rp0 = p;
    }
    Ok(())
  }

  /// Move indirect relative point.
  ///
  /// MIRP\[abcde\] (0xE0 - 0xFF)
  ///
  /// a: 0: do not set rp0 to point p after move
  ///    1: do set rp0 to point p after move
  /// b: 0: do not keep distance greater than or equal to minimum_distance
  ///    1: keep distance greater than or equal to minimum_distance
  /// c: 0: do not round distance and do not look at control_value_cutin
  ///    1: round the distance and look at control_value_cutin
  /// de: distance type for engine characteristic compensation
  ///
  /// Pops: n: CVT entry number
  ///       p: point number
  ///       
  /// A MIRP instruction makes it possible to preserve the distance between
  /// two points subject to a number of qualifications. Depending upon the
  /// setting of Boolean flag b, the distance can be kept greater than or
  /// equal to the value established by the minimum_distance state variable.
  /// Similarly, the instruction can be set to round the distance according
  /// to the round_state graphics state variable. The value of the minimum
  /// distance variable is the smallest possible value the distance between
  /// two points can be rounded to. Additionally, if the c Boolean is set,
  /// the MIRP instruction acts subject to the control_value_cut_in. If the
  /// difference between the actual measurement and the value in the CVT is
  /// sufficiently small (less than the cut_in_value), the CVT value will be
  /// used and not the actual value. If the device space difference between
  /// this distance from the CVT and the single_width_value is smaller than
  /// the single_width_cut_in, then use the single_width_value rather than
  /// the outline or Control Value Table distance.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#move-indirect-relative-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5731>
  pub(super) fn op_mirp(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let n = (self.value_stack.pop()?.wrapping_add(1)) as usize;
    let p = self.value_stack.pop_usize()?;
    if !gs.is_pedantic && (!gs.in_bounds([(gs.zp1, p), (gs.zp0, gs.rp0)]) || (n > self.cvt.len())) {
      gs.rp1 = gs.rp0;
      if (opcode & 16) != 0 {
        gs.rp0 = p;
      }
      gs.rp2 = p;
      return Ok(());
    }
    let mut cvt_distance = if n == 0 {
      F26Dot6::ZERO
    } else {
      self.cvt.get(n - 1)?
    };
    // single width test
    let cutin = gs.single_width_cutin;
    let value = gs.single_width;
    let mut delta = cvt_distance.wrapping_sub(value).abs();
    if delta < cutin {
      cvt_distance = if cvt_distance >= F26Dot6::ZERO {
        value
      } else {
        -value
      };
    }
    if gs.zp1.is_twilight() {
      let fv = gs.freedom_vector;
      let point = {
        let d = cvt_distance.to_bits();
        let p2 = gs.zp0().original(gs.rp0)?;
        let p1 = gs.zp1_mut().original_mut(p)?;
        p1.x = p2.x + F26Dot6::from_bits(math::mul(d, fv.x));
        p1.y = p2.y + F26Dot6::from_bits(math::mul(d, fv.y));
        *p1
      };
      *gs.zp1_mut().point_mut(p)? = point;
    }
    let original_distance = gs.dual_project(gs.zp1().original(p)?, gs.zp0().original(gs.rp0)?);
    let current_distance = gs.project(gs.zp1().point(p)?, gs.zp0().point(gs.rp0)?);
    // auto flip test
    if gs.auto_flip && (original_distance.to_bits() ^ cvt_distance.to_bits()) < 0 {
      cvt_distance = -cvt_distance;
    }
    if gs.target.gdi
      && gs.backward_compatibility
      && gs.zp0 == gs.zp1
      && (cvt_distance - original_distance).abs() > gs.gdi_cutin()
    {
      cvt_distance = original_distance;
    }
    // control value cutin and round
    let mut distance = if (opcode & 4) != 0 {
      if gs.zp0 == gs.zp1 {
        delta = cvt_distance.wrapping_sub(original_distance).abs();
        if delta > gs.gdi_cutin() {
          cvt_distance = original_distance;
        }
      }
      gs.round(cvt_distance)
    } else {
      cvt_distance
    };
    // minimum distance test
    if (opcode & 8) != 0 {
      let min_distance = gs.gdi_minimum();
      if original_distance >= F26Dot6::ZERO {
        if distance < min_distance {
          distance = min_distance
        };
      } else if distance > -min_distance {
        distance = -min_distance
      }
    }
    gs.move_point(gs.zp1, p, distance.wrapping_sub(current_distance))?;
    gs.link_width(gs.zp1, p, gs.zp0, gs.rp0);
    gs.stem_width(p, opcode & 3 == 1);
    gs.rp1 = gs.rp0;
    if (opcode & 16) != 0 {
      gs.rp0 = p;
    }
    gs.rp2 = p;
    Ok(())
  }

  /// Align relative point.
  ///
  /// ALIGNRP[] (0x3C)
  ///
  /// Pops: p: point number (uint32)
  ///
  /// Uses the loop counter.
  ///
  /// Reduces the distance between rp0 and point p to zero. Since distance
  /// is measured along the projection_vector and movement is along the
  /// freedom_vector, the effect of the instruction is to align points.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#align-relative-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5882>
  pub(super) fn op_alignrp(&mut self) -> OpResult {
    let gs = &mut self.graphics;
    let count = gs.loop_counter;
    gs.loop_counter = 1;
    for _ in 0..count {
      let p = self.value_stack.pop_usize()?;
      let distance = gs.project(gs.zp1().point(p)?, gs.zp0().point(gs.rp0)?);
      gs.move_point(gs.zp1, p, -distance)?;
      gs.link_width(gs.zp1, p, gs.zp0, gs.rp0);
    }
    Ok(())
  }

  /// Move point to intersection of two lines.
  ///
  /// ISECT[] (0x0F)
  ///
  /// Pops: b1: end point of line 2
  ///       b0: start point of line 2
  ///       a1: end point of line 1
  ///       a0: start point of line 1
  ///       p: point to move.
  ///
  /// Puts point p at the intersection of the lines A and B. The points a0
  /// and a1 define line A. Similarly, b0 and b1 define line B. ISECT
  /// ignores the freedom_vector in moving point p.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#moves-point-p-to-the-intersection-of-two-lines>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5934>
  pub(super) fn op_isect(&mut self) -> OpResult {
    let gs = &mut self.graphics;
    let b1 = self.value_stack.pop_usize()?;
    let b0 = self.value_stack.pop_usize()?;
    let a1 = self.value_stack.pop_usize()?;
    let a0 = self.value_stack.pop_usize()?;
    let point_ix = self.value_stack.pop_usize()?;
    // Lots of funky fixed point math so just map these to i32 to avoid
    // a bunch of wrapping/unwrapping.
    // To shreds you say!
    let [pa0, pa1] = {
      let z = gs.zp1();
      [z.point(a0)?, z.point(a1)?].map(|p| p.map(F26Dot6::to_bits))
    };
    let [pb0, pb1] = {
      let z = gs.zp0();
      [z.point(b0)?, z.point(b1)?].map(|p| p.map(F26Dot6::to_bits))
    };
    let dbx = pb1.x.wrapping_sub(pb0.x);
    let dby = pb1.y.wrapping_sub(pb0.y);
    let dax = pa1.x.wrapping_sub(pa0.x);
    let day = pa1.y.wrapping_sub(pa0.y);
    let dx = pb0.x.wrapping_sub(pa0.x);
    let dy = pb0.y.wrapping_sub(pa0.y);
    use math::mul_div;
    let discriminant = mul_div(dax, -dby, 0x40).wrapping_add(mul_div(day, dbx, 0x40));
    let dotproduct = mul_div(dax, dbx, 0x40).wrapping_add(mul_div(day, dby, 0x40));
    // Useful context from FreeType:
    //
    // "The discriminant above is actually a cross product of vectors
    // da and db. Together with the dot product, they can be used as
    // surrogates for sine and cosine of the angle between the vectors.
    // Indeed,
    //       dotproduct   = |da||db|cos(angle)
    //       discriminant = |da||db|sin(angle)
    // We use these equations to reject grazing intersections by
    // thresholding abs(tan(angle)) at 1/19, corresponding to 3 degrees."
    //
    // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L5986>
    if discriminant.wrapping_abs().wrapping_mul(19) > dotproduct.wrapping_abs() {
      let v = mul_div(dx, -dby, 0x40).wrapping_add(mul_div(dy, dbx, 0x40));
      let x = mul_div(v, dax, discriminant);
      let y = mul_div(v, day, discriminant);
      let point = gs.zp2_mut().point_mut(point_ix)?;
      point.x = F26Dot6::from_bits(pa0.x.wrapping_add(x));
      point.y = F26Dot6::from_bits(pa0.y.wrapping_add(y));
    } else {
      let point = gs.zp2_mut().point_mut(point_ix)?;
      point.x = F26Dot6::from_bits(
        (pa0
          .x
          .wrapping_add(pa1.x)
          .wrapping_add(pb0.x)
          .wrapping_add(pb1.x))
          / 4,
      );
      point.y = F26Dot6::from_bits(
        (pa0
          .y
          .wrapping_add(pa1.y)
          .wrapping_add(pb0.y)
          .wrapping_add(pb1.y))
          / 4,
      );
    }
    gs.zp2_mut().touch(point_ix, CoordAxis::Both)?;
    Ok(())
  }

  /// Align points.
  ///
  /// ALIGNPTS[] (0x27)
  ///
  /// Pops: p1: point number
  ///       p2: point number
  ///
  /// Makes the distance between point 1 and point 2 zero by moving both
  /// along the freedom_vector to the average of both their projections
  /// along the projection_vector.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#align-points>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L6030>
  pub(super) fn op_alignpts(&mut self) -> OpResult {
    let p2 = self.value_stack.pop_usize()?;
    let p1 = self.value_stack.pop_usize()?;
    let gs = &mut self.graphics;
    let distance = F26Dot6::from_bits(
      gs.project(gs.zp0().point(p2)?, gs.zp1().point(p1)?)
        .to_bits()
        / 2,
    );
    gs.move_point(gs.zp1, p1, distance)?;
    gs.move_point(gs.zp0, p2, -distance)?;
    Ok(())
  }

  /// Interpolate point by last relative stretch.
  ///
  /// IP[] (0x39)
  ///
  /// Pops: p: point number
  ///
  /// Uses the loop counter.
  ///
  /// Moves point p so that its relationship to rp1 and rp2 is the same as it
  /// was in the original uninstructed outline. Measurements are made along
  /// the projection_vector, and movement to satisfy the interpolation
  /// relationship is constrained to be along the freedom_vector. This
  /// instruction is not valid if rp1 and rp2 have the same position on the
  /// projection_vector.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#interpolate-point-by-the-last-relative-stretch>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L6065>
  pub(super) fn op_ip(&mut self) -> OpResult {
    let gs = &mut self.graphics;
    let count = gs.loop_counter;
    gs.loop_counter = 1;
    if !gs.is_pedantic && !gs.in_bounds([(gs.zp0, gs.rp1), (gs.zp1, gs.rp2)]) {
      return Ok(());
    }
    let in_twilight = gs.zp0.is_twilight() || gs.zp1.is_twilight() || gs.zp2.is_twilight();
    let orus_base = if in_twilight {
      gs.zp0().original(gs.rp1)?
    } else {
      gs.zp0().unscaled(gs.rp1).map(F26Dot6::from_bits)
    };
    let cur_base = gs.zp0().point(gs.rp1)?;
    let old_range = if in_twilight {
      gs.dual_project(gs.zp1().original(gs.rp2)?, orus_base)
    } else {
      gs.dual_project(gs.zp1().unscaled(gs.rp2).map(F26Dot6::from_bits), orus_base)
    };
    let cur_range = gs.project(gs.zp1().point(gs.rp2)?, cur_base);
    for _ in 0..count {
      let point = self.value_stack.pop_usize()?;
      if !gs.is_pedantic && !gs.in_bounds([(gs.zp2, point)]) {
        continue;
      }
      let original_distance = if in_twilight {
        gs.dual_project(gs.zp2().original(point)?, orus_base)
      } else {
        gs.dual_project(gs.zp2().unscaled(point).map(F26Dot6::from_bits), orus_base)
      };
      let cur_distance = gs.project(gs.zp2().point(point)?, cur_base);
      let new_distance = if original_distance != F26Dot6::ZERO {
        if old_range != F26Dot6::ZERO {
          F26Dot6::from_bits(math::mul_div(
            original_distance.to_bits(),
            cur_range.to_bits(),
            old_range.to_bits(),
          ))
        } else {
          original_distance
        }
      } else {
        F26Dot6::ZERO
      };
      gs.move_point(gs.zp2, point, new_distance.wrapping_sub(cur_distance))?;
      gs.interpolate_width(point, original_distance.to_bits(), old_range.to_bits());
    }
    Ok(())
  }

  /// Interpolate untouched points through the outline.
  ///
  /// IUP\[a\] (0x30 - 0x31)
  ///
  /// a: 0: interpolate in the y-direction
  ///    1: interpolate in the x-direction
  ///
  /// Considers a glyph contour by contour, moving any untouched points in
  /// each contour that are between a pair of touched points. If the
  /// coordinates of an untouched point were originally between those of
  /// the touched pair, it is linearly interpolated between the new
  /// coordinates, otherwise the untouched point is shifted by the amount
  /// the nearest touched point is shifted.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#interpolate-untouched-points-through-the-outline>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L6391>
  pub(super) fn op_iup(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let axis = if (opcode & 1) != 0 {
      CoordAxis::X
    } else {
      CoordAxis::Y
    };
    let mut run = true;
    // In backward compatibility mode, allow IUP until it has been done on
    // both axes.
    if gs.backward_compatibility {
      if gs.did_iup_x && gs.did_iup_y {
        run = false;
      }
      if axis == CoordAxis::X {
        gs.did_iup_x = true;
      } else {
        gs.did_iup_y = true;
      }
    }
    if run {
      if axis == CoordAxis::X {
        gs.compatible_width
          .apply(gs.zones[ZonePointer::Glyph as usize].points);
      }
      gs.zone_mut(ZonePointer::Glyph).iup(axis)?;
    }
    Ok(())
  }

  /// Untouch point.
  ///
  /// UTP[] (0x29)
  ///
  /// Pops: p: point number (uint32)
  ///
  /// Marks point p as untouched. A point may be touched in the x direction,
  /// the y direction, both, or neither. This instruction uses the current
  /// freedom_vector to determine whether to untouch the point in the
  /// x-direction, the y direction, or both. Points that are marked as
  /// untouched will be moved by an IUP (interpolate untouched points)
  /// instruction. Using UTP you can ensure that a point will be affected
  /// by IUP even if it was previously touched.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#untouch-point>
  /// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L6222>
  pub(super) fn op_utp(&mut self) -> OpResult {
    let p = self.value_stack.pop_usize()?;
    let coord_axis = match (
      self.graphics.freedom_vector.x != 0,
      self.graphics.freedom_vector.y != 0,
    ) {
      (true, true) => Some(CoordAxis::Both),
      (true, false) => Some(CoordAxis::X),
      (false, true) => Some(CoordAxis::Y),
      (false, false) => None,
    };
    if let Some(coord_axis) = coord_axis {
      self.graphics.zp0_mut().untouch(p, coord_axis)?;
    }
    Ok(())
  }

  /// Helper for FLIPRGON and FLIPRGOFF.
  fn set_on_curve_for_range(&mut self, on: bool) -> OpResult {
    let high_point = self.value_stack.pop_usize()?;
    let low_point = self.value_stack.pop_usize()?;
    // high_point is inclusive but Zone::set_on_curve takes an exclusive
    // range
    let high_point = high_point
      .checked_add(1)
      .ok_or(HintErrorKind::InvalidPointIndex(high_point))?;
    // In backward compatibility mode, don't flip points after IUP has
    // been done.
    if self.graphics.backward_compatibility && self.graphics.did_iup_x && self.graphics.did_iup_y {
      return Ok(());
    }
    self
      .graphics
      .zone_mut(ZonePointer::Glyph)
      .set_on_curve(low_point, high_point, on)
  }
}
