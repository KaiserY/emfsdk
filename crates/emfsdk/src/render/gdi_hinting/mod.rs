//! Classic GDI ClearType hinting for static TrueType glyphs.
//!
//! Skrifa's public interpreter target preserves horizontal outlines. GDI uses
//! a virtual horizontal grid and suppresses only delta-like moves. The private
//! interpreter adaptation and its provenance are described in README.md.
mod composite;
mod hint;

use super::{TinySkiaPath, TinySkiaPathBuilder};
use skrifa::raw::{
  FontRef, TableProvider,
  tables::{
    glyf::{CompositeGlyphFlags, Glyph, PointFlags},
    gvar::Gvar,
  },
  types::{F26Dot6, Fixed, Point, Tag},
};
use skrifa::{
  GlyphId, MetadataProvider,
  instance::{LocationRef, Size},
};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, Default)]
struct Target {
  gdi: bool,
  monochrome: bool,
  symmetric: bool,
  compatible_widths: bool,
}
impl Target {
  fn is_smooth(self) -> bool {
    !self.monochrome
  }
  fn preserve_linear_metrics(self) -> bool {
    false
  }
  fn is_vertical_lcd(self) -> bool {
    false
  }
  fn symmetric_rendering(self) -> bool {
    self.symmetric
  }
  fn is_grayscale_cleartype(self) -> bool {
    false
  }
}

struct Outlines<'a> {
  font: FontRef<'a>,
  fpgm: &'a [u8],
  prep: &'a [u8],
  gvar: Option<Gvar<'a>>,
  cvt_len: u32,
  max_function_defs: u16,
  max_instruction_defs: u16,
  max_twilight_points: u16,
  max_stack_elements: u16,
  max_storage: u16,
}
impl<'a> Outlines<'a> {
  fn new(font: &FontRef<'a>) -> Option<Self> {
    // Composite and variation execution stays with the existing scaler until
    // its component/variation contracts have native controls of their own.
    if font.fvar().is_ok() {
      return None;
    }
    font.glyf().ok()?;
    font.loca(None).ok()?;
    let maxp = font.maxp().ok()?;
    Some(Self {
      font: font.clone(),
      fpgm: font
        .table_data(Tag::new(b"fpgm"))
        .map_or(&[], |d| d.as_bytes()),
      prep: font
        .table_data(Tag::new(b"prep"))
        .map_or(&[], |d| d.as_bytes()),
      gvar: None,
      cvt_len: font.cvt().map_or(0, |t| t.len() as u32),
      max_function_defs: maxp.max_function_defs().unwrap_or_default(),
      max_instruction_defs: maxp.max_instruction_defs().unwrap_or_default(),
      max_twilight_points: maxp
        .max_twilight_points()
        .unwrap_or_default()
        .saturating_add(4),
      max_stack_elements: maxp
        .max_stack_elements()
        .unwrap_or_default()
        .saturating_add(32),
      max_storage: maxp.max_storage().unwrap_or_default(),
    })
  }
}

/// Cached horizontal device advances for static TrueType fonts in GDI Natural mode.
///
/// This measures at an integer pixel height with no transform or font simulation.
/// It does not shape text, add character spacing, or apply line justification.
/// Instruction-free composites can inherit an untransformed component with
/// `USE_MY_METRICS`. Instructed composites with offset-positioned, unit-axis
/// reflected children and their own metrics execute their parent program after
/// their children. Other composites, unsupported fonts and interpreter errors
/// return `None`;
/// callers must keep their existing fallback for those cases.
pub struct GdiNaturalMetrics {
  data: Arc<[u8]>,
  face_index: u32,
  hinting: GdiHinting,
  advances: HashMap<u32, Option<i32>>,
}

impl GdiNaturalMetrics {
  /// Prepares the font's control programs once for the specified pixel height.
  pub fn new(data: Arc<[u8]>, face_index: u32, ppem: u16) -> Option<Self> {
    let font = FontRef::from_index(&data, face_index).ok()?;
    // DirectWrite GDI Natural reports asymmetric smoothing, integer origin
    // positioning, and natural widths. Raster rendering selects its own mode.
    let hinting = GdiHinting::new(&font, f32::from(ppem), false, false)?;
    Some(Self {
      data,
      face_index,
      hinting,
      advances: HashMap::new(),
    })
  }

  /// Returns a glyph's hinted whole-pixel advance, including zero-width glyphs.
  pub fn glyph_advance_px(&mut self, glyph_id: u32) -> Option<i32> {
    if let Some(advance) = self.advances.get(&glyph_id) {
      return *advance;
    }
    let font = FontRef::from_index(&self.data, self.face_index).ok()?;
    let advance = self
      .hinting
      .natural_advance(&font, GlyphId::new(glyph_id), 0)
      .map(|advance| advance.round().to_i32());
    self.advances.insert(glyph_id, advance);
    advance
  }
}

struct HintedGlyph {
  points: Vec<Point<F26Dot6>>,
  flags: Vec<PointFlags>,
  contours: Vec<u16>,
  shift: F26Dot6,
  advance: F26Dot6,
}

pub(super) struct GdiHinting {
  instance: hint::HintInstance,
  monochrome: Option<hint::HintInstance>,
  ppem: f32,
}
impl GdiHinting {
  pub(super) fn new(
    font: &FontRef<'_>,
    ppem: f32,
    symmetric: bool,
    compatible_widths: bool,
  ) -> Option<Self> {
    if !ppem.is_finite() || ppem <= 0.0 || ppem > u16::MAX as f32 {
      return None;
    }
    let outlines = Outlines::new(font)?;
    let head = font.head().ok()?;
    let upem = head.units_per_em();
    if upem == 0 {
      return None;
    }
    let ppem = if head
      .flags()
      .contains(skrifa::raw::tables::head::Flags::FORCE_INTEGER_PPEM)
    {
      F26Dot6::from_f64(f64::from(ppem)).round().to_f32()
    } else {
      ppem
    };
    let scale = Fixed::from_bits((ppem * 64.0) as i32) / Fixed::from_bits(i32::from(upem));
    let rounded_ppem = ((scale * Fixed::from_bits(i32::from(upem))).to_bits() + 32) >> 6;
    let mut instance = hint::HintInstance::default();
    instance
      .reconfigure(
        &outlines,
        scale.to_bits(),
        rounded_ppem,
        Target {
          gdi: true,
          monochrome: false,
          symmetric,
          compatible_widths,
        },
        &[],
      )
      .ok()?;
    let monochrome = if compatible_widths {
      let mut mono = hint::HintInstance::default();
      mono
        .reconfigure(
          &outlines,
          scale.to_bits(),
          rounded_ppem,
          Target {
            monochrome: true,
            ..Default::default()
          },
          &[],
        )
        .ok()?;
      Some(mono)
    } else {
      None
    };
    instance.is_enabled().then_some(Self {
      instance,
      monochrome,
      ppem,
    })
  }

  pub(super) fn draw(&self, font: &FontRef<'_>, id: GlyphId) -> Option<TinySkiaPath> {
    let glyf = font.glyf().ok()?;
    let loca = font.loca(None).ok()?;
    let Glyph::Simple(glyph) = loca.get_glyf(id, &glyf).ok()?? else {
      return None;
    };
    if glyph.instructions().is_empty() {
      return None;
    }
    let hinted = self.hint_simple(font, id)?;
    path_from_points(
      &hinted.points,
      &hinted.flags,
      &hinted.contours,
      hinted.shift,
    )
  }

  fn natural_advance(&self, font: &FontRef<'_>, id: GlyphId, depth: usize) -> Option<F26Dot6> {
    // Bound malformed/cyclic component graphs just as the outline scaler does.
    if depth > 32 {
      return None;
    }
    let glyf = font.glyf().ok()?;
    let loca = font.loca(None).ok()?;
    if let Some(Glyph::Composite(glyph)) = loca.get_glyf(id, &glyf).ok()? {
      if glyph.instructions().is_some_and(|code| !code.is_empty()) {
        return self
          .hint_natural_composite(font, id, depth)
          .map(|glyph| glyph.advance);
      }
      let mut metrics_component = None;
      for component in glyph.components() {
        if component
          .flags
          .contains(CompositeGlyphFlags::USE_MY_METRICS)
        {
          if metrics_component.is_some() || component.transform != Default::default() {
            return None;
          }
          metrics_component = Some(component.glyph);
        }
      }
      // The inherited advance includes the component's instructions. Translation
      // and other decorative components do not change it. Do not substitute the
      // parent's hmtx width or run the component at the parent's advance.
      return self.natural_advance(font, metrics_component?.into(), depth + 1);
    }
    self.hint_simple(font, id).map(|glyph| glyph.advance)
  }

  fn hint_simple(&self, font: &FontRef<'_>, id: GlyphId) -> Option<HintedGlyph> {
    let glyf = font.glyf().ok()?;
    let loca = font.loca(None).ok()?;
    let glyph = loca.get_glyf(id, &glyf).ok()?;
    let instructions = match &glyph {
      Some(Glyph::Simple(glyph)) => glyph.instructions(),
      None => &[], // Empty glyphs still have horizontal metric phantom points.
      Some(Glyph::Composite(_)) => return None,
    };
    let outlines = Outlines::new(font)?;
    let glyphs = font.outline_glyphs();
    let outline = glyphs.get(id)?;
    let unscaled = outline
      .with_scaled_glyf_outline(Size::unscaled(), LocationRef::default(), None, |o| {
        // The public scaler has already applied the left phantom shift. The VM
        // consumes coordinates before that shift; apply it once after hinting.
        let shift = o.phantom_points[0].x;
        let mut points: Vec<_> = o
          .points
          .iter()
          .map(|p| Point::new((p.x + shift).to_i32(), p.y.to_i32()))
          .collect();
        points.extend(o.phantom_points.iter().map(|p| p.map(F26Dot6::to_i32)));
        Ok(points)
      })
      .ok()?;
    let (mut scaled, mut flags, contours, mut phantom) = outline
      .with_scaled_glyf_outline(Size::new(self.ppem), LocationRef::default(), None, |o| {
        let shift = o.phantom_points[0].x;
        let points: Vec<_> = o
          .points
          .iter()
          .map(|p| Point::new(p.x + shift, p.y))
          .collect();
        Ok((
          points,
          o.flags.to_vec(),
          o.contours.to_vec(),
          o.phantom_points,
        ))
      })
      .ok()?;
    if flags.iter().any(|f| f.is_off_curve_cubic()) {
      return None;
    }
    let count = scaled.len();
    scaled.extend_from_slice(&phantom);
    flags.resize(scaled.len(), PointFlags::default());
    let unhinted = scaled.clone();
    let unhinted_flags = flags.clone();
    let mut original = unhinted.clone();
    let mut stack = vec![0; outlines.max_stack_elements as usize];
    let mut cvt = vec![0; outlines.cvt_len as usize];
    let mut storage = vec![0; outlines.max_storage as usize];
    let mut twilight = vec![Point::default(); outlines.max_twilight_points as usize];
    let mut twilight_original = twilight.clone();
    let mut twilight_flags = vec![PointFlags::default(); twilight.len()];
    let mut compatible_advance = None;
    for (instance, mono) in self
      .monochrome
      .iter()
      .map(|h| (h, true))
      .chain(std::iter::once((&self.instance, false)))
    {
      scaled.copy_from_slice(&unhinted);
      original.copy_from_slice(&unhinted);
      flags.copy_from_slice(&unhinted_flags);
      phantom.copy_from_slice(&unhinted[count..]);
      for p in &mut scaled[count..] {
        p.x = if mono {
          p.x.round()
        } else {
          F26Dot6::from_bits(p.x.to_bits().saturating_add(2) & !3)
        };
        p.y = p.y.round();
      }
      instance
        .hint(
          &outlines,
          &mut hint::HintOutline {
            glyph_id: id,
            compatible_advance: if mono { None } else { compatible_advance },
            unscaled: &unscaled,
            scaled: &mut scaled,
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
            is_composite: false,
            coords: &[],
          },
          true,
        )
        .ok()?;
      if mono {
        // Keep F26.6 precision: public drawn metrics round to whole pixels.
        compatible_advance = Some(scaled[count + 1].x - scaled[count].x);
      }
    }
    let advance = scaled[count + 1].x - scaled[count].x;
    scaled.truncate(count);
    flags.truncate(count);
    Some(HintedGlyph {
      points: scaled,
      flags,
      contours,
      shift: phantom[0].x,
      advance,
    })
  }
}

fn path_from_points(
  points: &[Point<F26Dot6>],
  flags: &[PointFlags],
  contours: &[u16],
  shift: F26Dot6,
) -> Option<TinySkiaPath> {
  let mut b = TinySkiaPathBuilder::new();
  let mut start = 0;
  let midpoint = |a: Point<F26Dot6>, c: Point<F26Dot6>| {
    Point::new(
      F26Dot6::from_bits(a.x.to_bits().wrapping_add(c.x.to_bits()) / 2),
      F26Dot6::from_bits(a.y.to_bits().wrapping_add(c.y.to_bits()) / 2),
    )
  };
  for &last in contours {
    let end = usize::from(last) + 1;
    let mut contour: Vec<_> = points
      .get(start..end)?
      .iter()
      .zip(flags.get(start..end)?)
      .map(|(p, f)| (Point::new(p.x - shift, p.y), f.is_on_curve()))
      .collect();
    let &(first, on) = contour.first()?;
    let &(last, last_on) = contour.last()?;
    let first = if on {
      contour.remove(0).0
    } else if last_on {
      contour.pop()?.0
    } else {
      midpoint(first, last)
    };
    b.move_to(first.x.to_f32(), first.y.to_f32());
    contour.push((first, true));
    let mut i = 0;
    while let Some(&(p, on)) = contour.get(i) {
      if on {
        b.line_to(p.x.to_f32(), p.y.to_f32());
        i += 1;
      } else {
        let &(next, on) = contour.get(i + 1)?;
        let end = if on { next } else { midpoint(p, next) };
        b.quad_to(p.x.to_f32(), p.y.to_f32(), end.x.to_f32(), end.y.to_f32());
        i += if on { 2 } else { 1 };
      }
    }
    b.close();
    start = end;
  }
  b.finish()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn natural_advances_match_directwrite() {
    let data: Arc<[u8]> = Arc::from(include_bytes!("../testdata/natural-advances.ttf").as_slice());
    let mut instances = HashMap::new();
    for line in include_str!("../testdata/natural-advances.txt")
      .lines()
      .filter(|l| !l.starts_with('#'))
    {
      let values: Vec<u32> = line
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
      let ppem = values[0] as u16;
      let metrics = instances
        .entry(ppem)
        .or_insert_with(|| GdiNaturalMetrics::new(data.clone(), 0, ppem).unwrap());
      assert_eq!(
        metrics.glyph_advance_px(values[1]),
        Some(values[2] as i32),
        "ppem {ppem}, glyph {}",
        values[1]
      );
      assert_eq!(metrics.glyph_advance_px(values[1]), Some(values[2] as i32));
    }
    assert_eq!(instances.len(), 8);
    assert!(
      instances
        .values()
        .all(|instance| instance.advances.len() == 84)
    );
    assert!(GdiNaturalMetrics::new(data, 0, 0).is_none());
  }

  #[test]
  fn natural_composite_advances_inherit_native_component_metrics() {
    let data: Arc<[u8]> =
      Arc::from(include_bytes!("../testdata/natural-composites.ttf").as_slice());
    let mut instances = HashMap::new();
    for line in include_str!("../testdata/natural-composites.txt")
      .lines()
      .filter(|l| !l.starts_with('#'))
    {
      let values: Vec<i32> = line
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
      let ppem = values[0] as u16;
      let metrics = instances
        .entry(ppem)
        .or_insert_with(|| GdiNaturalMetrics::new(data.clone(), 0, ppem).unwrap());
      let expected = (values[2] >= 0).then_some(values[2]);
      assert_eq!(
        metrics.glyph_advance_px(values[1] as u32),
        expected,
        "ppem {ppem}, glyph {}",
        values[1]
      );
    }
  }

  #[test]
  fn natural_composite_programs_match_native_gdi() {
    let data: Arc<[u8]> =
      Arc::from(include_bytes!("../testdata/natural-composite-programs.ttf").as_slice());
    let mut instances = HashMap::new();
    let mut count = 0;
    for line in include_str!("../testdata/natural-composite-programs.txt")
      .lines()
      .filter(|line| !line.starts_with('#'))
    {
      let values: Vec<i32> = line
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
      let metrics = instances
        .entry(values[0] as u16)
        .or_insert_with(|| GdiNaturalMetrics::new(data.clone(), 0, values[0] as u16).unwrap());
      assert_eq!(
        metrics.glyph_advance_px(values[1] as u32),
        Some(values[2]),
        "ppem {}, glyph {}",
        values[0],
        values[1]
      );
      count += 1;
    }
    assert_eq!(count, 384);
  }

  #[test]
  fn natural_composite_phantoms_match_native_readback() {
    let data: Arc<[u8]> =
      Arc::from(include_bytes!("../testdata/natural-composite-phantoms.ttf").as_slice());
    let mut instances = HashMap::new();
    let mut count = 0;
    for line in include_str!("../testdata/natural-composite-phantoms.txt")
      .lines()
      .filter(|line| !line.starts_with('#'))
    {
      let values: Vec<i32> = line
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
      let metrics = instances
        .entry(values[0] as u16)
        .or_insert_with(|| GdiNaturalMetrics::new(data.clone(), 0, values[0] as u16).unwrap());
      assert_eq!(
        metrics.glyph_advance_px(values[1] as u32),
        Some(values[2]),
        "ppem {}, glyph {}",
        values[0],
        values[1]
      );
      count += 1;
    }
    assert_eq!(count, 32);
  }

  #[test]
  fn compatible_width_points_match_native_gdi() {
    check_native_points(
      include_bytes!("../testdata/cleartype-compatible-widths.ttf"),
      include_str!("../testdata/cleartype-compatible-widths.txt"),
    );
  }

  #[test]
  fn compatible_stem_points_match_native_gdi() {
    check_native_points(
      include_bytes!("../testdata/cleartype-compatible-stems.ttf"),
      include_str!("../testdata/cleartype-compatible-stems.txt"),
    );
  }

  #[test]
  fn projection_and_bearing_points_match_native_gdi() {
    check_native_points(
      include_bytes!("../testdata/cleartype-projection.ttf"),
      include_str!("../testdata/cleartype-projection.txt"),
    );
  }

  fn check_native_points(font: &[u8], expectations: &str) {
    let face = FontRef::new(font).unwrap();
    let mut failures = Vec::new();
    for line in expectations.lines().filter(|l| !l.starts_with('#')) {
      let mut fields = line.split_whitespace();
      let index: u32 = fields.next().unwrap().parse().unwrap();
      let quality: u8 = fields.next().unwrap().parse().unwrap();
      let name = fields.next().unwrap();
      let expected: Vec<i32> = fields.map(|v| v.parse().unwrap()).collect();
      let id = face
        .charmap()
        .map(char::from_u32(0xe000 + index).unwrap())
        .unwrap();
      let hinting = GdiHinting::new(&face, 64.0, true, quality == 5).unwrap();
      let path = hinting.draw(&face, id).unwrap();
      let actual: Vec<i32> = path.points()[..8]
        .iter()
        .map(|p| (p.x * 64.0) as i32)
        .collect();
      if actual != expected {
        failures.push(format!(
          "{name}, GDI quality {quality}: {actual:?} != {expected:?}"
        ));
      }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
  }
}
