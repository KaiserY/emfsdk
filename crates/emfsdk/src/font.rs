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
  if ppem < u16::from(start_ppem) || ppem > u16::from(end_ppem) {
    return None;
  }
  let entries_start = group_offset.checked_add(GROUP_HEADER_SIZE)?;
  let entries_end = entries_start.checked_add(record_count.checked_mul(ENTRY_SIZE)?)?;
  if entries_end > table.len() {
    return None;
  }
  for index in 0..record_count {
    let entry_offset = entries_start + index * ENTRY_SIZE;
    let entry_ppem = read_u16(entry_offset)?;
    if entry_ppem > ppem {
      break;
    }
    if entry_ppem == ppem {
      let y_max = i32::from(read_u16(entry_offset + 2)? as i16);
      let y_min = i32::from(read_u16(entry_offset + 4)? as i16);
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
