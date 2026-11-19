//! Word's fixed-output pen adapter, separate from GDI raster realization.
//!
//! Native opposite controls and the observed MSO pen-conversion call path
//! establish three independent owners: logical pen width, the 0.14pt output
//! minimum, and the classic narrow-solid-line endpoint flag. Extended pens
//! retain that last flag; their geometric caps still override it.

use super::*;
use crate::emf::{EmrCreatePen, EmrExtCreatePen, EmrPenEndCap, EmrPenJoin};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
  Classic,
  Geometric(MetafileVectorLineCap),
  Cosmetic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Pen {
  color: [u8; 3],
  width: u32,
  kind: Kind,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct State {
  cosmetic_line: bool,
  minimum_width: [f32; 2],
}

impl State {
  pub(super) fn new(physical: super::super::MetafilePhysicalSize) -> Option<Self> {
    (physical.width_pt.is_finite()
      && physical.height_pt.is_finite()
      && physical.width_pt > 0.0
      && physical.height_pt > 0.0)
      .then_some(Self {
        cosmetic_line: false,
        minimum_width: [0.14 / physical.width_pt, 0.14 / physical.height_pt],
      })
  }
}

pub(super) fn classic_pen(value: &EmrCreatePen) -> VectorPen {
  if value.pen_line_style_kind() == Some(EmrPenLineStyle::Null) {
    return VectorPen::Null;
  }
  if value.pen_line_style_kind() != Some(EmrPenLineStyle::Solid)
    || value.pen_reserved_bits() != 0
    || value.width.y != 0
  {
    return VectorPen::Unsupported;
  }
  let color = color(value.color);
  let width = value.width.x.unsigned_abs().max(1);
  if width == 1 {
    VectorPen::SolidCosmetic(color)
  } else {
    VectorPen::Word(Pen {
      color,
      width,
      kind: Kind::Classic,
    })
  }
}

pub(super) fn extended_pen(value: &EmrExtCreatePen) -> VectorPen {
  if value.pen_line_style_kind() == Some(EmrPenLineStyle::Null) {
    return VectorPen::Null;
  }
  if value.pen_reserved_bits() != 0
    || value.brush_style_kind() != Some(WmfBrushStyle::Solid)
    || value.bitmap_info_size != 0
    || value.bitmap_bits_size != 0
    || !value.style_entries.is_empty()
    || value.pen_join_kind() != Some(EmrPenJoin::Round)
  {
    return VectorPen::Unsupported;
  }
  let kind = match value.pen_type_kind() {
    Some(EmrPenType::Cosmetic)
      if value.width == 1
        && matches!(
          value.pen_line_style_kind(),
          Some(EmrPenLineStyle::Solid | EmrPenLineStyle::Alternate)
        ) =>
    {
      // MSO builds a zero-width LOGPEN from an extended cosmetic pen.
      // Its unmasked style has no dash case for PS_ALTERNATE; the output
      // is a solid minimum-width stroke, not recording-device stippling.
      Kind::Cosmetic
    }
    Some(EmrPenType::Geometric)
      if value.width > 0 && value.pen_line_style_kind() == Some(EmrPenLineStyle::Solid) =>
    {
      let cap = match value.pen_end_cap_kind() {
        Some(EmrPenEndCap::Flat) => MetafileVectorLineCap::Flat,
        Some(EmrPenEndCap::Round) => MetafileVectorLineCap::Round,
        Some(EmrPenEndCap::Square) => MetafileVectorLineCap::Square,
        None => return VectorPen::Unsupported,
      };
      Kind::Geometric(cap)
    }
    _ => return VectorPen::Unsupported,
  };
  VectorPen::Word(Pen {
    color: color(value.color),
    width: value.width,
    kind,
  })
}

struct Stroke {
  color: [u8; 3],
  width: [f32; 2],
  cap: MetafileVectorLineCap,
  cosmetic_line: bool,
}

fn stroke(interpreter: &mut VectorInterpreter) -> Option<Option<Stroke>> {
  let pen = match interpreter.graphics.pen {
    VectorPen::Null => return Some(None),
    VectorPen::Unsupported | VectorPen::WmfLogical { .. } => return None,
    VectorPen::SolidCosmetic(color) => Pen {
      color,
      width: 1,
      kind: Kind::Classic,
    },
    VectorPen::Word(pen) => pen,
  };
  if interpreter.graphics.binary_raster_operation != Some(WmfBinaryRasterOperation::CopyPen) {
    return None;
  }
  let state = interpreter.word_pens.as_mut()?;
  if pen.kind == Kind::Classic {
    state.cosmetic_line = pen.width <= 1;
  }
  let mapping = interpreter.graphics.mapping;
  let mut width = state.minimum_width;
  if pen.kind != Kind::Cosmetic {
    let origin = mapping.map_point(EmfPoint { x: 0, y: 0 })?;
    let end = mapping.map_point(EmfPoint {
      x: i32::try_from(pen.width).ok()?,
      y: 0,
    })?;
    let device_width = ((end.x - origin.x) * mapping.surface_width)
      .hypot((end.y - origin.y) * mapping.surface_height);
    width[0] = width[0].max(device_width / mapping.surface_width);
    width[1] = width[1].max(device_width / mapping.surface_height);
  }
  Some(Some(Stroke {
    color: pen.color,
    width,
    cap: match pen.kind {
      Kind::Geometric(cap) => cap,
      _ => MetafileVectorLineCap::Flat,
    },
    cosmetic_line: state.cosmetic_line,
  }))
}

pub(super) fn line(interpreter: &mut VectorInterpreter, start: EmfPoint, end: EmfPoint) -> bool {
  let Some(stroke) = stroke(interpreter) else {
    return false;
  };
  let Some(mut stroke) = stroke else {
    return true;
  };
  let centered = stroke.cosmetic_line && stroke.cap == MetafileVectorLineCap::Flat;
  let end = if centered {
    // Locate only the last logical GDI pixel, before the picture mapping.
    // The existing exact diamond-exit rule supplies its excluded endpoint.
    // A bounded clip keeps this constant work even for very long lines.
    let Some(end) = line_endpoint(start, end) else {
      return true;
    };
    stroke.cap = MetafileVectorLineCap::Square;
    end
  } else {
    end
  };
  let mapping = interpreter.graphics.mapping;
  let (Some(start), Some(end)) = (
    mapping.map_device_point(start, centered),
    mapping.map_device_point(end, centered),
  ) else {
    return false;
  };
  emit(interpreter, vec![start, end], stroke, false)
}

pub(super) fn line_endpoint(start: EmfPoint, end: EmfPoint) -> Option<EmfPoint> {
  let mut last = None;
  let dx = i128::from(end.x) - i128::from(start.x);
  let dy = i128::from(end.y) - i128::from(start.y);
  super::super::gdi_line::visit_cosmetic_line(
    (start.x as f32, start.y as f32),
    (end.x as f32, end.y as f32),
    (
      end.x.saturating_sub(2),
      end.y.saturating_sub(2),
      end.x.saturating_add(3),
      end.y.saturating_add(3),
    ),
    |x, y| {
      let position =
        (i128::from(x) - i128::from(start.x)) * dx + (i128::from(y) - i128::from(start.y)) * dy;
      if last.is_none_or(|(old, _)| position > old) {
        last = Some((position, EmfPoint { x, y }));
      }
    },
  );
  last.map(|(_, end)| end)
}

pub(super) fn rectangle(interpreter: &mut VectorInterpreter, rect: crate::types::RectL) -> bool {
  if interpreter.graphics.brush == VectorBrush::Unsupported {
    return false;
  }
  let Some(stroke) = stroke(interpreter) else {
    return false;
  };
  let mapping = interpreter.graphics.mapping;
  let Some(points) = [
    EmfPoint {
      x: rect.left,
      y: rect.top,
    },
    EmfPoint {
      x: rect.right,
      y: rect.top,
    },
    EmfPoint {
      x: rect.right,
      y: rect.bottom,
    },
    EmfPoint {
      x: rect.left,
      y: rect.bottom,
    },
  ]
  .into_iter()
  .map(|p| mapping.map_device_point(p, false))
  .collect::<Option<Vec<_>>>() else {
    return false;
  };
  let Some(drawing) = interpreter.drawing.as_mut() else {
    return false;
  };
  if let VectorBrush::Solid(color) = interpreter.graphics.brush {
    drawing.push(MetafileVectorDraw::Fill {
      fill: MetafileVectorFill {
        subpaths: vec![points.clone()],
        color,
        fill_rule: interpreter.graphics.fill_rule,
      },
      clip: interpreter.graphics.clip,
    });
  }
  stroke.is_none_or(|stroke| emit(interpreter, points, stroke, true))
}

fn emit(
  interpreter: &mut VectorInterpreter,
  points: Vec<MetafileVectorPoint>,
  stroke: Stroke,
  closed: bool,
) -> bool {
  let Some(drawing) = interpreter.drawing.as_mut() else {
    return false;
  };
  drawing.push(MetafileVectorDraw::Stroke {
    subpaths: vec![points],
    color: stroke.color,
    width: stroke.width,
    line_cap: stroke.cap,
    closed,
    clip: interpreter.graphics.clip,
  });
  true
}
