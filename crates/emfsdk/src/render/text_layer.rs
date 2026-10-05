//! Conservative GDI text-layer separation. Record ink bounds are measured in
//! the recording device; the accepted MM_TEXT DC keeps those coordinates.
//! This is a representability check, not a fallback rendering heuristic.

use std::collections::HashMap;

use crate::common::SdkEnumValue;
use crate::emf::{EmfMetafileRef, EmfRecordData, EmrComment, EmrMapMode, EmrPenLineStyle};
use crate::emfplus::{EmfPlusRecordData, EmfPlusRecordType, EmfPlusUnitType};
use crate::types::{ColorRef, PointL, RectL};
use crate::wmf::WmfBrushStyle;

#[derive(Clone, Copy)]
enum Object {
  Brush(Option<ColorRef>),
  Pen(Option<f32>),
  Font,
}

#[derive(Clone, Copy)]
struct Dc {
  brush: Option<ColorRef>,
  pen: Option<f32>,
  current: PointL,
  opaque: bool,
  background: ColorRef,
  font_selected: bool,
  clip: Option<Bounds>,
}

impl Default for Dc {
  fn default() -> Self {
    Self {
      brush: Some(color(0x00ff_ffff)),
      pen: Some(1.0),
      current: PointL { x: 0, y: 0 },
      opaque: true,
      background: color(0x00ff_ffff),
      font_selected: false,
      clip: None,
    }
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

pub(super) fn can_lift(data: &[u8]) -> bool {
  analyze(data).unwrap_or(false)
}

fn analyze(data: &[u8]) -> Option<bool> {
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
      EmfRecordData::SetMapMode(value) if value.map_mode == EmrMapMode::Text.raw() => {}
      EmfRecordData::SetWindowOrgEx(value) if value.origin == (PointL { x: 0, y: 0 }) => {}
      EmfRecordData::SetViewportOrgEx(value) if value.origin == (PointL { x: 0, y: 0 }) => {}
      EmfRecordData::SetWindowExtEx(_) | EmfRecordData::SetViewportExtEx(_) => {}
      EmfRecordData::SetRop2(value) if value.rop2_mode == 13 => {}
      EmfRecordData::SetPolyFillMode(_)
      | EmfRecordData::SelectPalette(_)
      | EmfRecordData::RealizePalette
      | EmfRecordData::SetTextColor(_) => {}
      EmfRecordData::IntersectClipRect(value) => {
        let rect = value.rect;
        let bounds = [
          rect.left as f32,
          rect.top as f32,
          rect.right as f32,
          rect.bottom as f32,
        ];
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
      EmfRecordData::SetTextAlign(value) if value.text_alignment_mode == 0 => {}
      EmfRecordData::SetBkMode(value) => dc.opaque = value.background_mode == 2,
      EmfRecordData::SetBkColor(value) => dc.background = value.color,
      EmfRecordData::SaveDc => saved.push(dc),
      EmfRecordData::RestoreDc(value) if value.saved_dc == -1 => dc = saved.pop()?,
      EmfRecordData::ExtCreateFontIndirectW(value) => {
        let font = value.font.log_font()?;
        if font.height >= 0
          || font
            .face_name
            .as_str()
            .ok()?
            .trim_end_matches('\0')
            .is_empty()
          || font.width != 0
          || font.escapement != font.orientation
          || font.underline != 0
          || font.strike_out != 0
        {
          return Some(false);
        }
        objects.insert(value.object_index, Object::Font);
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
        let pen = (value.pen_line_style_kind()? != EmrPenLineStyle::Null).then_some(width);
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
        0x8000_0006..=0x8000_0007 => dc.pen = Some(1.0),
        0x8000_0008 => dc.pen = None,
        0x8000_000a..=0x8000_000e | 0x8000_0010..=0x8000_0011 => dc.font_selected = false,
        index => match *objects.get(&index)? {
          Object::Brush(fill) => dc.brush = fill,
          Object::Pen(width) => dc.pen = width,
          Object::Font => dc.font_selected = true,
        },
      },
      EmfRecordData::DeleteObject(value) => {
        objects.remove(&value.object_index);
      }
      EmfRecordData::MoveToEx(value) => dc.current = value.point,
      EmfRecordData::LineTo(value) => {
        if let Some(width) = dc.pen {
          let margin = width * 0.5 + 1.0;
          painted.push((
            [
              dc.current.x.min(value.point.x) as f32 - margin,
              dc.current.y.min(value.point.y) as f32 - margin,
              dc.current.x.max(value.point.x) as f32 + margin,
              dc.current.y.max(value.point.y) as f32 + margin,
            ],
            None,
          ));
        }
        dc.current = value.point;
      }
      EmfRecordData::Rectangle(value) => {
        let b = edges(value.bounds);
        if let Some(color) = dc.brush {
          painted.push((b, Some(color)));
        }
        if let Some(width) = dc.pen {
          let m = width * 0.5 + 1.0;
          painted.extend([
            ([b[0] - m, b[1] - m, b[2] + m, b[1] + m], None),
            ([b[0] - m, b[3] - m, b[2] + m, b[3] + m], None),
            ([b[0] - m, b[1] - m, b[0] + m, b[3] + m], None),
            ([b[2] - m, b[1] - m, b[2] + m, b[3] + m], None),
          ]);
        }
      }
      EmfRecordData::Polygon16(value) | EmfRecordData::Polyline16(value) => {
        painted.push((edges(value.bounds), None))
      }
      EmfRecordData::ExtTextOutW(value) => {
        if !dc.font_selected
          || !value.text.options.is_empty()
          || !value.text.dx_buffer_present
          || value.text.dx.iter().any(|advance| (*advance as i32) < 0)
        {
          return Some(false);
        }
        let bounds = edges(value.bounds);
        if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
          return Some(false);
        }
        if dc.clip.is_some_and(|clip| !intersects(clip, bounds)) {
          return Some(false);
        }
        if dc.opaque
          && (text_bounds
            .iter()
            .any(|previous| intersects(*previous, bounds))
            || graphics.iter().any(|(previous, color)| {
              intersects(*previous, bounds) && *color != Some(dc.background)
            }))
        {
          return Some(false);
        }
        text_bounds.push(bounds);
      }
      _ => return Some(false),
    }
    if painted
      .iter()
      .any(|(bounds, _)| text_bounds.iter().any(|text| intersects(*bounds, *text)))
    {
      return Some(false);
    }
    graphics.extend(painted);
  }
  Some(!text_bounds.is_empty())
}
