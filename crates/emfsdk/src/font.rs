//! Device font metrics shared by metafile and document layout consumers.

/// Hinted ascent/descent in integer device pixels for one font size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GdiVerticalDeviceMetrics {
  pub ascent: i32,
  pub descent: i32,
}

/// Read the exact VDMX record for a square-pixel device and negative LOGFONT height.
///
/// `char_set` is the realized GDI character set. Missing sizes, incompatible
/// ratios/character sets, and malformed or truncated tables return `None`.
pub fn vdmx_vertical_device_metrics(
  table: &[u8],
  ppem: u16,
  char_set: u8,
) -> Option<GdiVerticalDeviceMetrics> {
  let (entries, start, end) = vdmx_entries(table, char_set)?;
  if ppem < u16::from(start) || ppem > u16::from(end) {
    return None;
  }
  for entry in entries.as_chunks::<6>().0 {
    let entry_ppem = u16::from_be_bytes([entry[0], entry[1]]);
    if entry_ppem > ppem {
      break;
    }
    if entry_ppem == ppem {
      let y_max = i32::from(i16::from_be_bytes([entry[2], entry[3]]));
      let y_min = i32::from(i16::from_be_bytes([entry[4], entry[5]]));
      if y_max <= 0 || y_min > 0 {
        return None;
      }
      return Some(GdiVerticalDeviceMetrics {
        ascent: y_max,
        descent: y_min.saturating_abs(),
      });
    }
  }
  None
}

fn vdmx_entries(table: &[u8], char_set: u8) -> Option<(&[u8], u8, u8)> {
  const ANSI_CHARSET: u8 = 0;
  const HEADER_SIZE: usize = 6;
  const RATIO_SIZE: usize = 4;
  const GROUP_HEADER_SIZE: usize = 4;
  const ENTRY_SIZE: usize = 6;

  let read_u16 = |offset: usize| {
    table
      .get(offset..offset + 2)
      .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
  };
  let version = read_u16(0)?;
  if version > 1 || read_u16(2)? == 0 {
    return None;
  }
  let ratio_count = usize::from(read_u16(4)?);
  let ratio_bytes = ratio_count.checked_mul(RATIO_SIZE)?;
  let offsets_start = HEADER_SIZE.checked_add(ratio_bytes)?;
  let offsets_end = offsets_start.checked_add(ratio_count.checked_mul(2)?)?;
  if offsets_end > table.len() {
    return None;
  }

  let mut group_offset = None;
  for index in 0..ratio_count {
    let ratio_offset = HEADER_SIZE + index * RATIO_SIZE;
    let ratio = table.get(ratio_offset..ratio_offset + RATIO_SIZE)?;
    let char_set_matches = match version {
      // Version 0 uses 1 for the Windows ANSI subset; 0 is the complete
      // symbol/dingbat repertoire. Microsoft specifies that Windows ignores
      // non-ANSI-subset entries for ANSI_CHARSET.
      0 => {
        (ratio[0] == 1 && char_set == ANSI_CHARSET) || (ratio[0] == 0 && char_set != ANSI_CHARSET)
      }
      // Version 1 uses 1 for the complete repertoire; 0 is additionally
      // available to ANSI_CHARSET consumers.
      1 => ratio[0] == 1 || (ratio[0] == 0 && char_set == ANSI_CHARSET),
      _ => false,
    };
    if !char_set_matches {
      continue;
    }
    let aspect_matches = (ratio[1] == 0 && ratio[2] == 0 && ratio[3] == 0)
      || (ratio[1] == 1 && ratio[2] <= 1 && ratio[3] >= 1);
    if aspect_matches {
      group_offset = Some(usize::from(read_u16(offsets_start + index * 2)?));
      break;
    }
  }

  let group_offset = group_offset?;
  let record_count = usize::from(read_u16(group_offset)?);
  let start_ppem = *table.get(group_offset + 2)?;
  let end_ppem = *table.get(group_offset + 3)?;
  let entries_start = group_offset.checked_add(GROUP_HEADER_SIZE)?;
  let entries_end = entries_start.checked_add(record_count.checked_mul(ENTRY_SIZE)?)?;
  if entries_end > table.len() {
    return None;
  }
  Some((&table[entries_start..entries_end], start_ppem, end_ppem))
}

/// Positive LOGFONT height selects the first matching device cell or the
/// preceding smaller entry. Above the table range the linear mapper applies.
pub(crate) fn vdmx_character_height_for_cell(
  table: &[u8],
  height: i32,
  char_set: u8,
) -> Option<u16> {
  let (entries, _, _) = vdmx_entries(table, char_set)?;
  let mut previous = None;
  for entry in entries.as_chunks::<6>().0 {
    let ppem = u16::from_be_bytes([entry[0], entry[1]]);
    let ascent = i32::from(i16::from_be_bytes([entry[2], entry[3]]));
    let bottom = i32::from(i16::from_be_bytes([entry[4], entry[5]]));
    if ascent <= 0 || bottom > 0 {
      return None;
    }
    match (ascent - bottom).cmp(&height) {
      std::cmp::Ordering::Equal => return Some(ppem),
      std::cmp::Ordering::Greater => return previous,
      std::cmp::Ordering::Less => previous = Some(ppem),
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn positive_cell_height_uses_hinted_boundaries_and_out_of_range_fallback() {
    // One ANSI square-pixel group, including equal cell heights at two ppems.
    let mut table = vec![0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 12, 0, 3, 8, 10];
    for (ppem, top, bottom) in [(8u16, 10i16, -2i16), (9, 11, -2), (10, 11, -2)] {
      table.extend_from_slice(&ppem.to_be_bytes());
      table.extend_from_slice(&top.to_be_bytes());
      table.extend_from_slice(&bottom.to_be_bytes());
    }
    assert_eq!(vdmx_character_height_for_cell(&table, 11, 0), None);
    assert_eq!(vdmx_character_height_for_cell(&table, 12, 0), Some(8));
    assert_eq!(vdmx_character_height_for_cell(&table, 13, 0), Some(9));
    assert_eq!(vdmx_character_height_for_cell(&table, 14, 0), None);
    assert_eq!(
      vdmx_character_height_for_cell(&table[..table.len() - 1], 13, 0),
      None
    );
    assert_eq!(
      vdmx_vertical_device_metrics(&table, 9, 0),
      Some(GdiVerticalDeviceMetrics {
        ascent: 11,
        descent: 2
      })
    );
  }
}
