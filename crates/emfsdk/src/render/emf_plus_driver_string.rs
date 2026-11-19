//! Positioned GDI+ glyphs, MS-EMFPLUS §§2.2.1.3 and 2.3.4.6.

use super::*;
use crate::emfplus::{
  EmfPlusDrawDriverStringData, EmfPlusDriverStringOptionsFlags, EmfPlusFontStyleFlags,
};
use skrifa::GlyphId;

pub(super) fn draw(value: &EmfPlusDrawDriverStringData, state: &mut EmfVectorState) {
  if state.suppress_text || value.glyphs.is_empty() {
    return;
  }
  let Some(brush) = emf_plus_brush_ref(value.brush, state) else {
    return;
  };
  let Some(Some(EmfPlusRenderObject::Font(font))) =
    state.emf_plus_objects.get(usize::from(value.font_id))
  else {
    return;
  };
  let Some(unit) = font.size_unit_kind() else {
    return;
  };
  let flags = font.font_style();
  // World/pixel fonts use the current graphics units. Physical fonts are
  // converted from their own unit independently of the graphics page scale.
  let em = match unit {
    EmfPlusUnitType::World | EmfPlusUnitType::Pixel => font.em_size,
    _ => {
      font.em_size
        * emf_plus_units_to_device_scale(
          unit,
          1.0,
          state.emf_plus_logical_dpi_y,
          state.emf_plus_video_display,
        )
        / state.emf_plus_page_device_scale().1
    }
  };
  if !em.is_finite() || em <= 0.0 {
    return;
  }
  let lookup = WmfTextFont {
    family: font.family_name.as_str().ok().map(|s| s.into_owned()),
    weight: if flags.contains(EmfPlusFontStyleFlags::BOLD) {
      700
    } else {
      400
    },
    italic: flags.contains(EmfPlusFontStyleFlags::ITALIC),
    width: 0,
    height: -1,
    escapement: 0,
    char_set: 0,
    quality: 0,
  };
  let Some(face_data) = state.font_cache.resolve_face(&lookup).cloned() else {
    return;
  };
  let Ok(face) = FontRef::from_index(face_data.font_data.as_ref(), face_data.face_index) else {
    return;
  };
  let Ok(head) = face.head() else { return };
  let units = f32::from(head.units_per_em());
  if units == 0.0 {
    return;
  }
  let location = face
    .axes()
    .location(face_data.synthesis.variation_settings().iter().copied());
  let metrics = face.glyph_metrics(FontSize::unscaled(), &location);
  let outlines = face.outline_glyphs();
  let cmap = face.charmap();
  let scale = em / units;
  let matrix = value
    .transform_matrix
    .map(xform_to_transform)
    .unwrap_or_else(EmfTransform::identity);
  // The driver's matrix changes each glyph about its supplied baseline
  // origin. The translation moves that origin too. Independent positions
  // do not acquire the glyph's width scale (native wide/step controls).
  let (ux, uy) = state.map_text_vector(scale * matrix.m11, scale * matrix.m12);
  let (vx, vy) = state.map_text_vector(-scale * matrix.m21, -scale * matrix.m22);
  let origin = state.map_text_point(EmfPoint { x: 0, y: 0 });
  let options = value.driver_string_options();
  let realized = options.contains(EmfPlusDriverStringOptionsFlags::REALIZED_ADVANCE);
  let Some(mut position) = value.glyph_positions.first().copied() else {
    return;
  };
  for (index, &word) in value.glyphs.iter().enumerate() {
    if !realized {
      position = value.glyph_positions[index];
    }
    let gid = if options.contains(EmfPlusDriverStringOptionsFlags::CMAP_LOOKUP) {
      char::from_u32(u32::from(word))
        .and_then(|ch| cmap.map(ch))
        .unwrap_or(GlyphId::NOTDEF)
    } else {
      GlyphId::new(u32::from(word))
    };
    let (dx, dy) = state.map_text_vector(position.x + matrix.dx, position.y + matrix.dy);
    let transform = TinySkiaTransform::from_row(ux, uy, vx, vy, origin.0 + dx, origin.1 + dy);
    let mut builder = TinySkiaPathBuilder::new();
    if let Some(outline) = outlines.get(gid)
      && outline
        .draw(
          DrawSettings::unhinted(FontSize::unscaled(), &location),
          &mut TinySkiaGlyphPathCollector {
            builder: &mut builder,
          },
        )
        .is_ok()
      && let Some(path) = builder.finish().and_then(|path| {
        synthesize_gdi_font_path(
          path,
          face_data.synthesis.skew(),
          (face_data.synthesis.embolden() && lookup.weight > 550).then_some(units / 24.0),
        )
      })
      && let Some(path) = path.transform(transform)
    {
      paint_outline(state, &path, &brush);
    }
    if realized {
      if options.contains(EmfPlusDriverStringOptionsFlags::VERTICAL) {
        let height = face
          .vmtx()
          .ok()
          .and_then(|v| v.advance(gid))
          .map_or(units, f32::from);
        position.x += height * scale * matrix.m21;
        position.y += height * scale * matrix.m22;
      } else {
        let width = metrics.advance_width(gid).unwrap_or_default() * scale;
        position.x += width * matrix.m11;
        position.y += width * matrix.m12;
      }
    }
  }
}

fn paint_outline(state: &mut EmfVectorState, path: &TinySkiaPath, brush: &EmfPlusRenderBrush) {
  let bounds = path.bounds();
  let canvas = (0, 0, state.width as i32, state.height as i32);
  let clip = state
    .clip_rect
    .map_or(canvas, |clip| intersect_rects(canvas, clip));
  let (left, top, right, bottom) = intersect_rects(
    clip,
    (
      bounds.left().floor() as i32,
      bounds.top().floor() as i32,
      bounds.right().ceil() as i32,
      bounds.bottom().ceil() as i32,
    ),
  );
  if right <= left || bottom <= top {
    return;
  }
  let Some(mut mask) = tiny_skia::Mask::new((right - left) as u32, (bottom - top) as u32) else {
    return;
  };
  mask.fill_path(
    path,
    tiny_skia::FillRule::Winding,
    true,
    TinySkiaTransform::from_translate(-(left as f32), -(top as f32)),
  );
  for (row, coverages) in mask.data().chunks_exact(mask.width() as usize).enumerate() {
    let y = top + row as i32;
    for (col, &coverage) in coverages
      .iter()
      .enumerate()
      .filter(|(_, coverage)| **coverage != 0)
    {
      let x = left + col as i32;
      let ink = brush.color_at(x, y);
      let alpha = ((u16::from(ink.alpha) * u16::from(coverage) + 127) / 255) as u8;
      state.set_pixel_with_alpha(x, y, ink.color, alpha);
    }
  }
}
