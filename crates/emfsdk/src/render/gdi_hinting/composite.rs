//! Natural metrics for instructed composites with explicitly positioned children.
//!
//! OpenType glyf requires children to be hinted before their transform/placement,
//! then executes the parent program over the assembled points. The preparation
//! below follows skrifa's glyf::Scaler::load_composite (MIT, bundled LICENSE-MIT).
//! Inherited metrics, point anchors and offset rounding keep their existing
//! fallback until their GDI contracts have independent native controls.
use super::{GdiHinting, HintedGlyph, Outlines, hint};
use skrifa::{
  GlyphId, MetadataProvider,
  instance::{LocationRef, Size},
  raw::{
    FontRef, TableProvider,
    tables::glyf::{Anchor, CompositeGlyphFlags, Glyph, PointFlags, PointMarker},
    types::{F26Dot6, Fixed, Point},
  },
};

impl GdiHinting {
  pub(super) fn hint_natural_composite(
    &self,
    font: &FontRef<'_>,
    id: GlyphId,
    depth: usize,
  ) -> Option<HintedGlyph> {
    if depth > 32 || self.monochrome.is_some() {
      return None;
    }
    let glyf = font.glyf().ok()?;
    let loca = font.loca(None).ok()?;
    let Glyph::Composite(glyph) = loca.get_glyf(id, &glyf).ok()?? else {
      return self.hint_simple(font, id);
    };
    let instructions = glyph.instructions().filter(|code| !code.is_empty())?;
    let upem = font.head().ok()?.units_per_em();
    let scale = Fixed::from_bits((self.ppem * 64.0) as i32) / Fixed::from_bits(i32::from(upem));
    let mut points = Vec::<Point<F26Dot6>>::new();
    let mut flags = Vec::<PointFlags>::new();
    let mut contours = Vec::<u16>::new();
    for component in glyph.components() {
      if component.flags.intersects(
        CompositeGlyphFlags::USE_MY_METRICS
          | CompositeGlyphFlags::ROUND_XY_TO_GRID
          | CompositeGlyphFlags::SCALED_COMPONENT_OFFSET,
      ) {
        return None;
      }
      let Anchor::Offset { x, y } = component.anchor else {
        return None;
      };
      let transform = component.transform;
      // Native scaling changes the child's instruction coordinate owner. Unit
      // axis reflections preserve that owner; general scaled/rotated children
      // need a separate GDI transform interpreter, not a post-hint affine map.
      if !matches!(transform.xx.to_bits(), -16384 | 16384)
        || !matches!(transform.yy.to_bits(), -16384 | 16384)
        || transform.xy.to_bits() != 0
        || transform.yx.to_bits() != 0
      {
        return None;
      }
      let child = self.hint_natural_composite(font, component.glyph.into(), depth + 1)?;
      let base = u16::try_from(points.len()).ok()?;
      let point_count = points.len().checked_add(child.points.len())?;
      u16::try_from(point_count).ok()?;
      let xx = Fixed::from_bits(i32::from(transform.xx.to_bits()) * 4);
      let xy = Fixed::from_bits(i32::from(transform.xy.to_bits()) * 4);
      let yx = Fixed::from_bits(i32::from(transform.yx.to_bits()) * 4);
      let yy = Fixed::from_bits(i32::from(transform.yy.to_bits()) * 4);
      let offset = Point::new(x, y)
        .map(|v| F26Dot6::from_bits((scale * Fixed::from_bits(i32::from(v))).to_bits()));
      for point in child.points {
        let point = point.map(|v| Fixed::from_bits(v.to_bits()));
        points.push(Point::new(
          F26Dot6::from_bits((point.x * xx + point.y * xy).to_bits()) + offset.x,
          F26Dot6::from_bits((point.x * yx + point.y * yy).to_bits()) + offset.y,
        ));
      }
      flags.extend(child.flags);
      for contour in child.contours {
        contours.push(contour.checked_add(base)?);
      }
    }
    // No child owns this parent's metrics. The public unhinted scaler supplies
    // its own hmtx/bearing phantom points, without guessing from child advances.
    let glyphs = font.outline_glyphs();
    let outline = glyphs.get(id)?;
    let mut phantom = outline
      .with_scaled_glyf_outline(Size::new(self.ppem), LocationRef::default(), None, |o| {
        Ok(o.phantom_points)
      })
      .ok()?;
    // Native parent-program GC readbacks show an already realized left
    // phantom, while the original right phantom preserves the unrounded
    // advance from that origin. Snap the origin and translate both bearings
    // together before capturing the original points; independent snapping
    // would alter the original metric width by a fraction of a pixel.
    let origin = F26Dot6::from_bits(phantom[0].x.to_bits().saturating_add(2) & !3);
    let offset = origin - phantom[0].x;
    phantom[0].x = origin;
    phantom[1].x += offset;
    let count = points.len();
    points.extend_from_slice(&phantom);
    flags.resize(points.len(), PointFlags::default());
    for flag in &mut flags {
      flag.clear_marker(PointMarker::TOUCHED);
    }
    // Composite programs see the already hinted/placed child points as their
    // original coordinates, in 26.6 bits, not the children's design positions.
    let unscaled: Vec<_> = points
      .iter()
      .map(|point| point.map(F26Dot6::to_bits))
      .collect();
    let mut original = points.clone();
    for point in &mut points[count..] {
      // GDI Natural uses the same 1/16px horizontal phantom grid as simple
      // glyphs. Native GC readback controls distinguish current from original
      // phantom coordinates; whole-pixel preparation loses bearing fractions.
      point.x = F26Dot6::from_bits(point.x.to_bits().saturating_add(2) & !3);
      point.y = point.y.round();
    }
    let outlines = Outlines::new(font)?;
    let mut stack = vec![0; outlines.max_stack_elements as usize];
    let mut cvt = vec![0; outlines.cvt_len as usize];
    let mut storage = vec![0; outlines.max_storage as usize];
    let mut twilight = vec![Point::default(); outlines.max_twilight_points as usize];
    let mut twilight_original = twilight.clone();
    let mut twilight_flags = vec![PointFlags::default(); twilight.len()];
    self
      .instance
      .hint(
        &outlines,
        &mut hint::HintOutline {
          glyph_id: id,
          compatible_advance: None,
          unscaled: &unscaled,
          scaled: &mut points,
          original_scaled: &mut original,
          flags: &mut flags,
          contours: &contours,
          phantom: &mut phantom,
          bytecode: instructions,
          stack: &mut stack,
          cvt: &mut cvt,
          storage: &mut storage,
          twilight_scaled: &mut twilight,
          twilight_original_scaled: &mut twilight_original,
          twilight_flags: &mut twilight_flags,
          is_composite: true,
          coords: &[],
        },
        true,
      )
      .ok()?;
    let advance = points[count + 1].x - points[count].x;
    points.truncate(count);
    flags.truncate(count);
    Some(HintedGlyph {
      points,
      flags,
      contours,
      shift: phantom[0].x,
      advance,
    })
  }
}
