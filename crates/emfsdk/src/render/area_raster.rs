//! Safe 24.8 fixed-point cell/area rasterization for grayscale glyph masks.
//!
//! The sweep follows the non-zero winding construction used by FreeType's
//! `ftgrays`: directed edges accumulate signed vertical cover and first moment
//! per cell, then a left-to-right prefix reconstructs byte area coverage. The
//! implementation is entirely safe Rust and keeps curve subdivision on the
//! same fixed-point boundary as the rasterizer.

use tiny_skia::{Path, PathSegment, Point};

const PIXEL_BITS: u32 = 8;
const ONE_PIXEL: i64 = 1_i64 << PIXEL_BITS;

#[derive(Clone, Copy, Debug, Default)]
struct FixedPoint {
  x: i64,
  y: i64,
}

impl FixedPoint {
  fn from_point(point: Point) -> Option<Self> {
    Some(Self {
      x: to_fixed(point.x)?,
      y: to_fixed(point.y)?,
    })
  }
}

fn to_fixed(value: f32) -> Option<i64> {
  let value = f64::from(value) * ONE_PIXEL as f64;
  (value.is_finite() && value >= i64::MIN as f64 && value <= i64::MAX as f64)
    .then_some(value.trunc() as i64)
}

fn trunc(value: i64) -> i32 {
  (value >> PIXEL_BITS) as i32
}

fn fraction(value: i64) -> i64 {
  value & (ONE_PIXEL - 1)
}

#[derive(Clone, Copy, Debug, Default)]
struct Cell {
  x: i32,
  cover: i64,
  area: i64,
}

struct Rasterizer {
  width: i32,
  height: i32,
  rows: Vec<Vec<Cell>>,
  start: FixedPoint,
  current: FixedPoint,
  cell_x: i32,
  cell_y: i32,
  cell_cover: i64,
  cell_area: i64,
  cell_valid: bool,
  contour_open: bool,
}

impl Rasterizer {
  fn new(width: u32, height: u32) -> Option<Self> {
    let width = i32::try_from(width).ok()?;
    let height = i32::try_from(height).ok()?;
    Some(Self {
      width,
      height,
      rows: vec![Vec::new(); usize::try_from(height).ok()?],
      start: FixedPoint::default(),
      current: FixedPoint::default(),
      cell_x: 0,
      cell_y: 0,
      cell_cover: 0,
      cell_area: 0,
      cell_valid: false,
      contour_open: false,
    })
  }

  fn move_to(&mut self, point: FixedPoint) {
    if self.contour_open {
      self.line_to(self.start);
    }
    self.set_cell(trunc(point.x), trunc(point.y));
    self.start = point;
    self.current = point;
    self.contour_open = true;
  }

  fn close(&mut self) {
    if self.contour_open {
      self.line_to(self.start);
      self.contour_open = false;
    }
  }

  fn flush_cell(&mut self) {
    if self.cell_valid && (self.cell_area != 0 || self.cell_cover != 0) {
      self.rows[self.cell_y as usize].push(Cell {
        x: self.cell_x,
        cover: self.cell_cover,
        area: self.cell_area,
      });
    }
    self.cell_cover = 0;
    self.cell_area = 0;
  }

  fn set_cell(&mut self, x: i32, y: i32) {
    self.flush_cell();
    self.cell_x = x.max(-1);
    self.cell_y = y;
    self.cell_valid = y >= 0 && y < self.height && x < self.width;
  }

  fn line_to(&mut self, point: FixedPoint) {
    let to_x = point.x;
    let to_y = point.y;
    let mut row = trunc(self.current.y);
    let target_row = trunc(to_y);
    if (row >= self.height && target_row >= self.height) || (row < 0 && target_row < 0) {
      self.current = point;
      return;
    }

    let mut column = trunc(self.current.x);
    let target_column = trunc(to_x);
    let mut fraction_x = fraction(self.current.x);
    let mut fraction_y = fraction(self.current.y);
    let delta_x = to_x - self.current.x;
    let delta_y = to_y - self.current.y;

    if column == target_column && row == target_row {
      // The remainder below accounts for the complete segment.
    } else if delta_y == 0 {
      self.set_cell(target_column, target_row);
      self.current = point;
      return;
    } else if delta_x == 0 {
      if delta_y > 0 {
        loop {
          let next_fraction_y = ONE_PIXEL;
          self.cell_cover += next_fraction_y - fraction_y;
          self.cell_area += (next_fraction_y - fraction_y) * fraction_x * 2;
          fraction_y = 0;
          row += 1;
          self.set_cell(column, row);
          if row == target_row {
            break;
          }
        }
      } else {
        loop {
          let next_fraction_y = 0;
          self.cell_cover += next_fraction_y - fraction_y;
          self.cell_area += (next_fraction_y - fraction_y) * fraction_x * 2;
          fraction_y = ONE_PIXEL;
          row -= 1;
          self.set_cell(column, row);
          if row == target_row {
            break;
          }
        }
      }
    } else {
      let mut product = delta_x * fraction_y - delta_y * fraction_x;
      let reciprocal_x = if column != target_column {
        0x00ff_ffff / delta_x
      } else {
        0
      };
      let reciprocal_y = if row != target_row {
        0x00ff_ffff / delta_y
      } else {
        0
      };
      loop {
        if product <= 0 && product - delta_x * ONE_PIXEL > 0 {
          let next_fraction_x = 0;
          let next_fraction_y = reciprocal_multiply(-product, -reciprocal_x);
          product -= delta_y * ONE_PIXEL;
          self.cell_cover += next_fraction_y - fraction_y;
          self.cell_area += (next_fraction_y - fraction_y) * (fraction_x + next_fraction_x);
          fraction_x = ONE_PIXEL;
          fraction_y = next_fraction_y;
          column -= 1;
        } else if product - delta_x * ONE_PIXEL <= 0
          && product - delta_x * ONE_PIXEL + delta_y * ONE_PIXEL > 0
        {
          product -= delta_x * ONE_PIXEL;
          let next_fraction_x = reciprocal_multiply(-product, reciprocal_y);
          let next_fraction_y = ONE_PIXEL;
          self.cell_cover += next_fraction_y - fraction_y;
          self.cell_area += (next_fraction_y - fraction_y) * (fraction_x + next_fraction_x);
          fraction_x = next_fraction_x;
          fraction_y = 0;
          row += 1;
        } else if product - delta_x * ONE_PIXEL + delta_y * ONE_PIXEL <= 0
          && product + delta_y * ONE_PIXEL >= 0
        {
          product += delta_y * ONE_PIXEL;
          let next_fraction_x = ONE_PIXEL;
          let next_fraction_y = reciprocal_multiply(product, reciprocal_x);
          self.cell_cover += next_fraction_y - fraction_y;
          self.cell_area += (next_fraction_y - fraction_y) * (fraction_x + next_fraction_x);
          fraction_x = 0;
          fraction_y = next_fraction_y;
          column += 1;
        } else {
          let next_fraction_x = reciprocal_multiply(product, -reciprocal_y);
          let next_fraction_y = 0;
          product += delta_x * ONE_PIXEL;
          self.cell_cover += next_fraction_y - fraction_y;
          self.cell_area += (next_fraction_y - fraction_y) * (fraction_x + next_fraction_x);
          fraction_x = next_fraction_x;
          fraction_y = ONE_PIXEL;
          row -= 1;
        }
        self.set_cell(column, row);
        if column == target_column && row == target_row {
          break;
        }
      }
    }

    let final_fraction_x = fraction(to_x);
    let final_fraction_y = fraction(to_y);
    self.cell_cover += final_fraction_y - fraction_y;
    self.cell_area += (final_fraction_y - fraction_y) * (fraction_x + final_fraction_x);
    self.current = point;
  }

  fn quad_to(&mut self, control: FixedPoint, point: FixedPoint) {
    let mut arc = [FixedPoint::default(); 33];
    arc[0] = point;
    arc[1] = control;
    arc[2] = self.current;
    if entirely_outside_vertical_clip(&arc[..3], self.height) {
      self.current = point;
      return;
    }

    let mut curvature = (arc[2].x + arc[0].x - 2 * arc[1].x)
      .abs()
      .max((arc[2].y + arc[0].y - 2 * arc[1].y).abs());
    let mut remaining = 1_i32;
    while curvature > ONE_PIXEL / 4 {
      curvature >>= 2;
      remaining <<= 1;
    }
    let mut offset = 0_usize;
    loop {
      let mut split = remaining & remaining.wrapping_neg();
      loop {
        split >>= 1;
        if split == 0 {
          break;
        }
        split_quad(&mut arc[offset..]);
        offset += 2;
      }
      self.line_to(arc[offset]);
      remaining -= 1;
      if remaining == 0 {
        break;
      }
      offset -= 2;
    }
  }

  fn curve_to(&mut self, control1: FixedPoint, control2: FixedPoint, point: FixedPoint) {
    let mut arc = [FixedPoint::default(); 129];
    arc[0] = point;
    arc[1] = control2;
    arc[2] = control1;
    arc[3] = self.current;
    if entirely_outside_vertical_clip(&arc[..4], self.height) {
      self.current = point;
      return;
    }

    let mut offset = 0_usize;
    loop {
      let requires_split = (2 * arc[offset].x - 3 * arc[offset + 1].x + arc[offset + 3].x).abs()
        > ONE_PIXEL / 2
        || (2 * arc[offset].y - 3 * arc[offset + 1].y + arc[offset + 3].y).abs() > ONE_PIXEL / 2
        || (arc[offset].x - 3 * arc[offset + 2].x + 2 * arc[offset + 3].x).abs() > ONE_PIXEL / 2
        || (arc[offset].y - 3 * arc[offset + 2].y + 2 * arc[offset + 3].y).abs() > ONE_PIXEL / 2;
      if requires_split {
        if arc.len() - offset >= 7 {
          split_cubic(&mut arc[offset..]);
          offset += 3;
          continue;
        }
        self.line_to(point);
        return;
      }
      self.line_to(arc[offset]);
      if offset == 0 {
        return;
      }
      offset -= 3;
    }
  }

  fn finish(mut self) -> Vec<u8> {
    self.close();
    self.flush_cell();
    let mut alpha = vec![0; self.width as usize * self.height as usize];
    for (y, row) in self.rows.iter_mut().enumerate() {
      row.sort_unstable_by_key(|cell| cell.x);
      let row_start = y * self.width as usize;
      let mut cover = 0_i64;
      let mut x = 0_i32;
      let mut index = 0;
      while index < row.len() {
        let cell_x = row[index].x;
        if cover != 0 && cell_x > x {
          let end = cell_x.min(self.width);
          if end > x {
            alpha[row_start + x as usize..row_start + end as usize].fill(nonzero_alpha(cover));
          }
        }

        let mut cell_cover = 0_i64;
        let mut cell_area = 0_i64;
        while index < row.len() && row[index].x == cell_x {
          cell_cover += row[index].cover;
          cell_area += row[index].area;
          index += 1;
        }
        cover += cell_cover * ONE_PIXEL * 2;
        let area = cover - cell_area;
        if area != 0 && cell_x >= 0 && cell_x < self.width {
          alpha[row_start + cell_x as usize] = nonzero_alpha(area);
        }
        x = cell_x + 1;
      }
      if cover != 0 && x < self.width {
        alpha[row_start + x.max(0) as usize..row_start + self.width as usize]
          .fill(nonzero_alpha(cover));
      }
    }
    alpha
  }
}

fn reciprocal_multiply(value: i64, reciprocal: i64) -> i64 {
  debug_assert!(value >= 0 && reciprocal >= 0);
  ((i128::from(value) * i128::from(reciprocal)) >> (32 - PIXEL_BITS)) as i64
}

fn entirely_outside_vertical_clip(points: &[FixedPoint], height: i32) -> bool {
  points.iter().all(|point| trunc(point.y) >= height)
    || points.iter().all(|point| trunc(point.y) < 0)
}

fn split_quad(points: &mut [FixedPoint]) {
  let mut a;
  let mut b;
  points[4].x = points[2].x;
  a = points[0].x + points[1].x;
  b = points[1].x + points[2].x;
  points[3].x = b >> 1;
  points[2].x = (a + b) >> 2;
  points[1].x = a >> 1;
  points[4].y = points[2].y;
  a = points[0].y + points[1].y;
  b = points[1].y + points[2].y;
  points[3].y = b >> 1;
  points[2].y = (a + b) >> 2;
  points[1].y = a >> 1;
}

fn split_cubic(points: &mut [FixedPoint]) {
  let mut a;
  let mut b;
  let mut c;
  points[6].x = points[3].x;
  a = points[0].x + points[1].x;
  b = points[1].x + points[2].x;
  c = points[2].x + points[3].x;
  points[5].x = c >> 1;
  c += b;
  points[4].x = c >> 2;
  points[1].x = a >> 1;
  a += b;
  points[2].x = a >> 2;
  points[3].x = (a + c) >> 3;
  points[6].y = points[3].y;
  a = points[0].y + points[1].y;
  b = points[1].y + points[2].y;
  c = points[2].y + points[3].y;
  points[5].y = c >> 1;
  c += b;
  points[4].y = c >> 2;
  points[1].y = a >> 1;
  a += b;
  points[2].y = a >> 2;
  points[3].y = (a + c) >> 3;
}

fn nonzero_alpha(mut area: i64) -> u8 {
  area >>= PIXEL_BITS * 2 + 1 - 8;
  if area < 0 {
    area = !area;
  }
  area.min(255) as u8
}

pub(super) fn rasterize_nonzero_path(path: &Path, width: u32, height: u32) -> Option<Vec<u8>> {
  if width == 0 || height == 0 {
    return None;
  }
  let _ = usize::try_from(width)
    .ok()?
    .checked_mul(usize::try_from(height).ok()?)?;
  let mut rasterizer = Rasterizer::new(width, height)?;
  for segment in path.segments() {
    match segment {
      PathSegment::MoveTo(point) => rasterizer.move_to(FixedPoint::from_point(point)?),
      PathSegment::LineTo(point) => rasterizer.line_to(FixedPoint::from_point(point)?),
      PathSegment::QuadTo(control, point) => {
        rasterizer.quad_to(
          FixedPoint::from_point(control)?,
          FixedPoint::from_point(point)?,
        );
      }
      PathSegment::CubicTo(control1, control2, point) => rasterizer.curve_to(
        FixedPoint::from_point(control1)?,
        FixedPoint::from_point(control2)?,
        FixedPoint::from_point(point)?,
      ),
      PathSegment::Close => rasterizer.close(),
    }
  }
  Some(rasterizer.finish())
}
