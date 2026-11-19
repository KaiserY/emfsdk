//! Conservative GDI text-layer separation. Record ink bounds are measured in
//! the recording device. Logical graphics and clips are mapped to that same plane.
//! This is a representability check, not a fallback rendering heuristic.

use std::collections::HashMap;

use crate::common::SdkEnumValue;
use crate::emf::{
  EmfMetafileRef, EmfRecordData, EmrComment, EmrMapMode, EmrPenEndCap, EmrPenJoin, EmrPenLineStyle,
  EmrPenType, ExtTextOutOptions,
};
use crate::emfplus::{EmfPlusRecordData, EmfPlusRecordType, EmfPlusUnitType};
use crate::types::{ColorRef, PointL, RectL};
use crate::wmf::{WmfBrushStyle, WmfTernaryRasterOperationCode};

#[derive(Clone, Copy)]
enum Object {
  Brush(Option<ColorRef>),
  Pen(Option<Pen>),
  Font { compatible_only: bool },
}

#[derive(Clone, Copy)]
struct Pen {
  width: f32,
  logical: bool,
  word_cosmetic: bool,
  color: ColorRef,
}

#[derive(Clone, Copy)]
struct Dc {
  brush: Option<ColorRef>,
  pen: Option<Pen>,
  current: PointL,
  opaque: bool,
  background: ColorRef,
  text_color: ColorRef,
  font_selected: bool,
  font_compatible_only: bool,
  extents: super::emf_mapping::Extents,
  window_origin: [i32; 2],
  viewport_origin: [i32; 2],
  clip: Option<Bounds>,
}

impl Default for Dc {
  fn default() -> Self {
    Self {
      brush: Some(color(0x00ff_ffff)),
      pen: Some(Pen {
        width: 1.0,
        logical: false,
        word_cosmetic: false,
        color: color(0),
      }),
      current: PointL { x: 0, y: 0 },
      opaque: true,
      background: color(0x00ff_ffff),
      text_color: color(0),
      font_selected: false,
      font_compatible_only: false,
      extents: super::emf_mapping::Extents {
        mode: EmrMapMode::Text,
        window: [1; 2],
        viewport: [1; 2],
      },
      window_origin: [0; 2],
      viewport_origin: [0; 2],
      clip: None,
    }
  }
}

impl Dc {
  fn scale(self) -> [f32; 2] {
    let (x, y) = super::emf_window_viewport_scale(
      self.extents.mode,
      self.extents.window[0],
      self.extents.window[1],
      self.extents.viewport[0],
      self.extents.viewport[1],
    );
    [x, y]
  }
  fn point(self, point: PointL) -> [f32; 2] {
    let scale = self.scale();
    [
      self.viewport_origin[0] as f32 + (point.x as f32 - self.window_origin[0] as f32) * scale[0],
      self.viewport_origin[1] as f32 + (point.y as f32 - self.window_origin[1] as f32) * scale[1],
    ]
  }
  fn rectangle(self, rect: RectL) -> Bounds {
    let a = self.point(PointL {
      x: rect.left,
      y: rect.top,
    });
    let b = self.point(PointL {
      x: rect.right,
      y: rect.bottom,
    });
    [
      a[0].min(b[0]),
      a[1].min(b[1]),
      a[0].max(b[0]),
      a[1].max(b[1]),
    ]
  }
  fn pen_margin(self, pen: Pen) -> f32 {
    let scale = self.scale();
    let width = if pen.logical {
      pen.width * scale[0].abs().max(scale[1].abs())
    } else {
      pen.width
    };
    width * 0.5 + 1.0
  }
}

type Bounds = [f32; 4];

fn color(raw: u32) -> ColorRef {
  ColorRef {
    red: raw as u8,
    green: (raw >> 8) as u8,
    blue: (raw >> 16) as u8,
    reserved: 0,
  }
}

fn edges(bounds: RectL) -> Bounds {
  [
    bounds.left as f32,
    bounds.top as f32,
    bounds.right as f32 + 1.0,
    bounds.bottom as f32 + 1.0,
  ]
}

fn intersects(a: Bounds, b: Bounds) -> bool {
  a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

fn contains(outer: Bounds, inner: Bounds) -> bool {
  outer[0] <= inner[0] && outer[1] <= inner[1] && outer[2] >= inner[2] && outer[3] >= inner[3]
}

pub(super) fn can_lift(data: &[u8], word_minimum: Option<f32>, logical_font_metrics: bool) -> bool {
  analyze(data, word_minimum, logical_font_metrics).unwrap_or(false)
}

fn analyze(data: &[u8], word_minimum: Option<f32>, logical_font_metrics: bool) -> Option<bool> {
  let metafile = EmfMetafileRef::from_bytes(data).ok()?;
  let mut dc = Dc::default();
  let mut saved = Vec::new();
  let mut objects = HashMap::new();
  let mut text_bounds = Vec::new();
  let mut graphics: Vec<(Bounds, Option<ColorRef>)> = Vec::new();
  let mut emf_plus = false;
  let mut get_dc = false;
  for record in metafile.records() {
    let parsed = record.parse_data().ok()?;
    if let EmfRecordData::Comment(EmrComment::EmfPlus { records, .. }) = &parsed {
      // MS-EMFPLUS 1.3 and 2.3.3.2: only GetDC's classical interval is
      // consumed by EMF+ playback. Never lift the independent Dual fallback.
      get_dc = false;
      for record in records {
        match record.record_kind()? {
          EmfPlusRecordType::Header => emf_plus = true,
          EmfPlusRecordType::GetDc => {
            get_dc = true;
            dc = Dc::default();
            saved.clear();
          }
          EmfPlusRecordType::SetPageTransform => {
            let EmfPlusRecordData::SetPageTransform(page) = record.parse_data().ok()? else {
              return Some(false);
            };
            let unit = record.flags().bits() & 0xff;
            if page.page_scale != 1.0
              || !matches!(
                EmfPlusUnitType::from_raw(u32::from(unit)),
                Some(EmfPlusUnitType::Display | EmfPlusUnitType::Pixel)
              )
            {
              return Some(false);
            }
          }
          EmfPlusRecordType::Eof
          | EmfPlusRecordType::SetAntiAliasMode
          | EmfPlusRecordType::SetTextRenderingHint
          | EmfPlusRecordType::SetTextContrast
          | EmfPlusRecordType::SetInterpolationMode
          | EmfPlusRecordType::SetPixelOffsetMode
          | EmfPlusRecordType::SetCompositingQuality
          | EmfPlusRecordType::SetWorldTransform => {}
          _ => return Some(false),
        }
      }
      continue;
    }
    if emf_plus && !get_dc {
      if matches!(parsed, EmfRecordData::Header(_) | EmfRecordData::Eof(_)) {
        continue;
      }
      // The semantic extractor does not currently discard Dual fallback
      // strings. Keep those streams entirely on their established path.
      return Some(false);
    }
    let mut painted = Vec::new();
    match parsed {
      EmfRecordData::Header(_) | EmfRecordData::Eof(_) => {}
      EmfRecordData::SetMapMode(_)
      | EmfRecordData::SetWindowExtEx(_)
      | EmfRecordData::SetViewportExtEx(_)
      | EmfRecordData::ScaleWindowExtEx(_)
      | EmfRecordData::ScaleViewportExtEx(_) => {
        dc.extents = dc
          .extents
          .record_payload(data, record.record_type, record.data);
      }
      EmfRecordData::SetWindowOrgEx(value) => dc.window_origin = [value.origin.x, value.origin.y],
      EmfRecordData::SetViewportOrgEx(value) => {
        dc.viewport_origin = [value.origin.x, value.origin.y]
      }
      EmfRecordData::SetRop2(value) if value.rop2_mode == 13 => {}
      EmfRecordData::SetPolyFillMode(_)
      | EmfRecordData::SelectPalette(_)
      | EmfRecordData::RealizePalette => {}
      EmfRecordData::SetTextColor(value) => dc.text_color = value.color,
      EmfRecordData::IntersectClipRect(value) => {
        let bounds = dc.rectangle(value.rect);
        dc.clip = Some(if let Some(previous) = dc.clip {
          [
            previous[0].max(bounds[0]),
            previous[1].max(bounds[1]),
            previous[2].min(bounds[2]),
            previous[3].min(bounds[3]),
          ]
        } else {
          bounds
        });
      }
      EmfRecordData::ExtSelectClipRgn(value) => {
        dc.clip = super::emf_mapping::copied_rectangle(&value)?
          .map(|rect| super::emf_mapping::device_edges(rect, [0.0; 2], [1.0; 2]));
      }
      // The extractor consumes ordinary horizontal/vertical alignment.
      // Record ink bounds already include that displacement and escapement.
      // Keep current-position, RTL and unknown modes outside this layer.
      EmfRecordData::SetTextAlign(value)
        if matches!(value.text_alignment_mode & !0x18, 0 | 2 | 6)
          && matches!(value.text_alignment_mode & 0x18, 0 | 8 | 24) => {}
      EmfRecordData::SetBkMode(value) => dc.opaque = value.background_mode == 2,
      EmfRecordData::SetBkColor(value) => dc.background = value.color,
      EmfRecordData::SaveDc => saved.push(dc),
      EmfRecordData::RestoreDc(value) if value.saved_dc == -1 => dc = saved.pop()?,
      EmfRecordData::ExtCreateFontIndirectW(value) => {
        let font = value.font.log_font()?;
        if font.height == 0
          || (font.height > 0 && !logical_font_metrics)
          || font
            .face_name
            .as_str()
            .ok()?
            .trim_end_matches('\0')
            .is_empty()
          || font.width != 0
          || font.underline != 0
          || font.strike_out != 0
        {
          return Some(false);
        }
        objects.insert(
          value.object_index,
          Object::Font {
            compatible_only: font.escapement != font.orientation,
          },
        );
      }
      EmfRecordData::CreateBrushIndirect(value) => {
        let fill = match WmfBrushStyle::from_raw(value.brush_style as u16)? {
          WmfBrushStyle::Solid => Some(value.color),
          WmfBrushStyle::Null => None,
          _ => return Some(false),
        };
        objects.insert(value.object_index, Object::Brush(fill));
      }
      EmfRecordData::CreatePen(value) => {
        let width = (value.width.x.unsigned_abs().max(1)) as f32;
        let pen = (value.pen_line_style_kind()? != EmrPenLineStyle::Null).then_some(Pen {
          width,
          logical: value.width.x != 0,
          word_cosmetic: false,
          color: value.color,
        });
        objects.insert(value.object_index, Object::Pen(pen));
      }
      EmfRecordData::ExtCreatePen(value) => {
        let pen = if value.pen_line_style_kind()? == EmrPenLineStyle::Null {
          None
        } else {
          // Only bound paint here; the graphics retain their own renderer.
          // Round/flat caps and round/bevel joins stay within half the width.
          // Miter/square extensions and bitmap brushes need wider analysis.
          if !matches!(
            value.pen_end_cap_kind()?,
            EmrPenEndCap::Round | EmrPenEndCap::Flat
          ) || !matches!(
            value.pen_join_kind()?,
            EmrPenJoin::Round | EmrPenJoin::Bevel
          ) || value.brush_style_kind()? != WmfBrushStyle::Solid
            || value.pen_reserved_bits() != 0
            || value.bitmap_info_size != 0
            || value.bitmap_bits_size != 0
          {
            return Some(false);
          }
          Some(Pen {
            width: value.width.max(1) as f32,
            logical: value.pen_type_kind()? == EmrPenType::Geometric,
            word_cosmetic: value.pen_type_kind()? == EmrPenType::Cosmetic,
            color: value.color,
          })
        };
        objects.insert(value.object_index, Object::Pen(pen));
      }
      EmfRecordData::SelectObject(value) => match value.object_index {
        0x8000_0000..=0x8000_0004 => {
          dc.brush = Some(color(match value.object_index {
            0x8000_0000 => 0x00ff_ffff,
            0x8000_0004 => 0,
            _ => return Some(false),
          }))
        }
        0x8000_0005 => dc.brush = None,
        0x8000_0006..=0x8000_0007 => {
          dc.pen = Some(Pen {
            width: 1.0,
            logical: false,
            word_cosmetic: false,
            color: color(if value.object_index == 0x8000_0006 {
              0x00ff_ffff
            } else {
              0
            }),
          })
        }
        0x8000_0008 => dc.pen = None,
        0x8000_000a..=0x8000_000e | 0x8000_0010..=0x8000_0011 => dc.font_selected = false,
        index => match *objects.get(&index)? {
          Object::Brush(fill) => dc.brush = fill,
          Object::Pen(width) => dc.pen = width,
          Object::Font { compatible_only } => {
            dc.font_selected = true;
            dc.font_compatible_only = compatible_only;
          }
        },
      },
      EmfRecordData::DeleteObject(value) => {
        objects.remove(&value.object_index);
      }
      EmfRecordData::MoveToEx(value) => dc.current = value.point,
      EmfRecordData::LineTo(value) => {
        if let Some(pen) = dc.pen {
          let margin = dc.pen_margin(pen);
          let a = dc.point(dc.current);
          let b = dc.point(value.point);
          let bounds = if let Some(width) = word_minimum.filter(|_| pen.word_cosmetic) {
            // Word's extended cosmetic pen is a flat .14pt stroke (native
            // pen adapter controls). Its cap does not extend past the end
            // along the baseline. The old 1.5px square envelope can overlap
            // a neighbouring text box even though the stroke cannot.
            let dx = b[0] - a[0];
            let dy = b[1] - a[1];
            let length = dx.hypot(dy);
            let (mx, my) = if length > 0.0 {
              (
                dy.abs() / length * width * 0.5,
                dx.abs() / length * width * 0.5,
              )
            } else {
              (0.0, 0.0)
            };
            [
              a[0].min(b[0]) - mx,
              a[1].min(b[1]) - my,
              a[0].max(b[0]) + mx,
              a[1].max(b[1]) + my,
            ]
          } else {
            [
              a[0].min(b[0]) - margin,
              a[1].min(b[1]) - margin,
              a[0].max(b[0]) + margin,
              a[1].max(b[1]) + margin,
            ]
          };
          painted.push((bounds, word_minimum.map(|_| pen.color)));
        }
        dc.current = value.point;
      }
      EmfRecordData::Rectangle(value) => {
        let b = dc.rectangle(value.bounds);
        if let Some(color) = dc.brush {
          painted.push((b, Some(color)));
        }
        if let Some(pen) = dc.pen {
          let m = dc.pen_margin(pen);
          let color = word_minimum.map(|_| pen.color);
          painted.extend([
            ([b[0] - m, b[1] - m, b[2] + m, b[1] + m], color),
            ([b[0] - m, b[3] - m, b[2] + m, b[3] + m], color),
            ([b[0] - m, b[1] - m, b[0] + m, b[3] + m], color),
            ([b[2] - m, b[1] - m, b[2] + m, b[3] + m], color),
          ]);
        }
      }
      EmfRecordData::Polygon16(value) | EmfRecordData::Polyline16(value) => {
        painted.push((edges(value.bounds), None))
      }
      EmfRecordData::BitBlt(value)
        if value.raster_operation_code() == WmfTernaryRasterOperationCode::PATCOPY
          && value.bitmap.is_none()
          && value.dest_size.cx > 0
          && value.dest_size.cy > 0 =>
      {
        let color = dc.brush?;
        let bounds = dc.rectangle(RectL {
          left: value.dest.x,
          top: value.dest.y,
          right: value.dest.x.checked_add(value.dest_size.cx)?,
          bottom: value.dest.y.checked_add(value.dest_size.cy)?,
        });
        painted.push((bounds, Some(color)));
      }
      EmfRecordData::ExtTextOutW(value) => {
        if !dc.font_selected
          || (dc.font_compatible_only && value.graphics_mode != 1)
          || !(value.text.options - ExtTextOutOptions::CLIPPED).is_empty()
          || !value.text.dx_buffer_present
          || value.text.dx.iter().any(|advance| (*advance as i32) < 0)
        {
          return Some(false);
        }
        let bounds = edges(value.bounds);
        if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
          return Some(false);
        }
        // ETO_CLIPPED is harmless only when its exclusive rectangle fully
        // contains the recorded ink. Partial per-record clips are not yet
        // carried by the semantic extractor; keep those on raster replay.
        if value.text.options.contains(ExtTextOutOptions::CLIPPED) {
          let rect = value.text.rectangle?;
          if !contains(dc.rectangle(rect), bounds) {
            return Some(false);
          }
        }
        if dc.clip.is_some_and(|clip| !intersects(clip, bounds)) {
          return Some(false);
        }
        if dc.opaque
          && (text_bounds
            .iter()
            .any(|(previous, _)| intersects(*previous, bounds))
            || graphics.iter().any(|(previous, color)| {
              intersects(*previous, bounds) && *color != Some(dc.background)
            }))
        {
          return Some(false);
        }
        text_bounds.push((bounds, (!dc.opaque).then_some(dc.text_color)));
      }
      _ => return Some(false),
    }
    // Only visible graphics can cover an already lifted string. Clip bounds
    // belong to the recording device, just like EMR_EXTTEXTOUTW.Bounds.
    if let Some(clip) = dc.clip {
      for (bounds, _) in &mut painted {
        bounds[0] = bounds[0].max(clip[0]);
        bounds[1] = bounds[1].max(clip[1]);
        bounds[2] = bounds[2].min(clip[2]);
        bounds[3] = bounds[3].min(clip[3]);
      }
      painted.retain(|(b, _)| b[0] < b[2] && b[1] < b[3]);
    }
    if painted.iter().any(|(bounds, color)| {
      text_bounds.iter().any(|(text, text_color)| {
        // Source-over paints of the same color commute: their combined
        // coverage is a + b - a*b. Word's transparent same-color text/line
        // order controls confirm identical PDF pixels; different colors or
        // an opaque text background do not. Require the complete Word vector
        // route, since raster text may carry colored ClearType coverage.
        let commutes = word_minimum.is_some() && color.is_some() && color == text_color;
        intersects(*bounds, *text) && !commutes
      })
    }) {
      return Some(false);
    }
    graphics.extend(painted);
  }
  Some(!text_bounds.is_empty())
}
