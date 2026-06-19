use std::io::Cursor;

use emfsdk_derive::SdkEnum;

use crate::common::{Error, Reader, Result, SdkEnumValue, SdkRead, SdkSize, SdkWrite, Writer};

pub const META_EOF: u16 = 0x0000;
pub const PLACEABLE_KEY: u32 = 0x9AC6_CDD7;
pub const PLACEABLE_HEADER_SIZE: usize = 22;
pub const WMF_HEADER_SIZE: usize = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u16")]
pub enum WmfRecordFunction {
    Eof = 0x0000,
    RealizePalette = 0x0035,
    SetPalEntries = 0x0037,
    CreatePalette = 0x00F7,
    SetBkMode = 0x0102,
    SetMapMode = 0x0103,
    SetRop2 = 0x0104,
    SetRelabs = 0x0105,
    SetPolyFillMode = 0x0106,
    SetStretchBltMode = 0x0107,
    SetTextCharExtra = 0x0108,
    RestoreDc = 0x0127,
    InvertRegion = 0x012A,
    PaintRegion = 0x012B,
    SelectClipRegion = 0x012C,
    SelectObject = 0x012D,
    SetTextAlign = 0x012E,
    ResizePalette = 0x0139,
    DibCreatePatternBrush = 0x0142,
    SetLayout = 0x0149,
    DeleteObject = 0x01F0,
    CreatePatternBrush = 0x01F9,
    CreatePenIndirect = 0x02FA,
    CreateFontIndirect = 0x02FB,
    CreateBrushIndirect = 0x02FC,
    SetBkColor = 0x0201,
    SetTextColor = 0x0209,
    SetTextJustification = 0x020A,
    SetWindowOrg = 0x020B,
    SetWindowExt = 0x020C,
    SetViewportOrg = 0x020D,
    SetViewportExt = 0x020E,
    OffsetWindowOrg = 0x020F,
    OffsetViewportOrg = 0x0211,
    LineTo = 0x0213,
    MoveTo = 0x0214,
    OffsetClipRgn = 0x0220,
    FillRegion = 0x0228,
    SetMapperFlags = 0x0231,
    SelectPalette = 0x0234,
    Polygon = 0x0324,
    Polyline = 0x0325,
    AnimatePalette = 0x0436,
    SetPixel = 0x041F,
    ExcludeClipRect = 0x0415,
    IntersectClipRect = 0x0416,
    Ellipse = 0x0418,
    FloodFill = 0x0419,
    Rectangle = 0x041B,
    ScaleWindowExt = 0x0410,
    ScaleViewportExt = 0x0412,
    FrameRegion = 0x0429,
    TextOut = 0x0521,
    PolyPolygon = 0x0538,
    ExtFloodFill = 0x0548,
    RoundRect = 0x061C,
    PatBlt = 0x061D,
    Escape = 0x0626,
    CreateRegion = 0x06FF,
    Arc = 0x0817,
    Pie = 0x081A,
    Chord = 0x0830,
    BitBlt = 0x0922,
    DibBitBlt = 0x0940,
    ExtTextOut = 0x0A32,
    StretchBlt = 0x0B23,
    DibStretchBlt = 0x0B41,
    SetDibToDev = 0x0D33,
    StretchDib = 0x0F43,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfMetafile {
    pub placeable_header: Option<WmfPlaceableHeader>,
    pub header: WmfHeader,
    pub records: Vec<WmfRecord>,
}

impl WmfMetafile {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(bytes));
        let placeable_header = if has_placeable_header(bytes) {
            Some(WmfPlaceableHeader::read_from(&mut reader)?)
        } else {
            None
        };
        let header = WmfHeader::read_from(&mut reader)?;
        let mut records = Vec::new();

        while reader.position()? < bytes.len() as u64 {
            let record = WmfRecord::read_from(&mut reader, bytes.len() as u64)?;
            let is_eof = record.function == META_EOF;
            records.push(record);
            if is_eof {
                break;
            }
        }

        Ok(Self {
            placeable_header,
            header,
            records,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        if let Some(header) = &self.placeable_header {
            header.write_to(&mut writer)?;
        }
        self.header.write_to(&mut writer)?;
        for record in &self.records {
            record.write_to(&mut writer)?;
        }
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfPlaceableHeader {
    pub key: u32,
    pub handle: u16,
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
    pub inch: u16,
    pub reserved: u32,
    pub checksum: u16,
}

impl WmfPlaceableHeader {
    pub fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        let offset = reader.position()?;
        let key = reader.read_u32()?;
        if key != PLACEABLE_KEY {
            return Err(Error::invalid(offset, "invalid WMF placeable header key"));
        }
        Ok(Self {
            key,
            handle: reader.read_u16()?,
            left: reader.read_i16()?,
            top: reader.read_i16()?,
            right: reader.read_i16()?,
            bottom: reader.read_i16()?,
            inch: reader.read_u16()?,
            reserved: reader.read_u32()?,
            checksum: reader.read_u16()?,
        })
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        writer.write_u32(self.key)?;
        writer.write_u16(self.handle)?;
        writer.write_i16(self.left)?;
        writer.write_i16(self.top)?;
        writer.write_i16(self.right)?;
        writer.write_i16(self.bottom)?;
        writer.write_u16(self.inch)?;
        writer.write_u32(self.reserved)?;
        writer.write_u16(self.checksum)
    }
}

impl SdkRead for WmfPlaceableHeader {
    fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        Self::read_from(reader)
    }
}

impl SdkWrite for WmfPlaceableHeader {
    fn write_to<W: std::io::Write + std::io::Seek>(&self, writer: &mut Writer<W>) -> Result<()> {
        self.write_to(writer)
    }
}

impl SdkSize for WmfPlaceableHeader {
    fn sdk_size(&self) -> u64 {
        PLACEABLE_HEADER_SIZE as u64
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfHeader {
    pub metafile_type: u16,
    pub header_size_words: u16,
    pub version: u16,
    pub file_size_words: u32,
    pub number_of_objects: u16,
    pub max_record_words: u32,
    pub number_of_parameters: u16,
}

impl WmfHeader {
    pub fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        let offset = reader.position()?;
        let header = Self {
            metafile_type: reader.read_u16()?,
            header_size_words: reader.read_u16()?,
            version: reader.read_u16()?,
            file_size_words: reader.read_u32()?,
            number_of_objects: reader.read_u16()?,
            max_record_words: reader.read_u32()?,
            number_of_parameters: reader.read_u16()?,
        };
        if header.header_size_words != 9 {
            return Err(Error::invalid(offset, "WMF header size must be 9 WORDs"));
        }
        Ok(header)
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        writer.write_u16(self.metafile_type)?;
        writer.write_u16(self.header_size_words)?;
        writer.write_u16(self.version)?;
        writer.write_u32(self.file_size_words)?;
        writer.write_u16(self.number_of_objects)?;
        writer.write_u32(self.max_record_words)?;
        writer.write_u16(self.number_of_parameters)
    }
}

impl SdkRead for WmfHeader {
    fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        Self::read_from(reader)
    }
}

impl SdkWrite for WmfHeader {
    fn write_to<W: std::io::Write + std::io::Seek>(&self, writer: &mut Writer<W>) -> Result<()> {
        self.write_to(writer)
    }
}

impl SdkSize for WmfHeader {
    fn sdk_size(&self) -> u64 {
        WMF_HEADER_SIZE as u64
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfRecord {
    pub function: u16,
    pub data: Vec<u8>,
}

impl WmfRecord {
    pub fn new(function: u16, data: Vec<u8>) -> Self {
        Self { function, data }
    }

    pub fn function_kind(&self) -> Option<WmfRecordFunction> {
        WmfRecordFunction::from_raw(self.function)
    }

    pub fn read_from<R: std::io::Read + std::io::Seek>(
        reader: &mut Reader<R>,
        file_len: u64,
    ) -> Result<Self> {
        let offset = reader.position()?;
        let size_words = reader.read_u32()?;
        let function = reader.read_u16()?;
        let size_bytes = size_words
            .checked_mul(2)
            .ok_or_else(|| Error::invalid(offset, "WMF record size overflows"))?;
        if size_bytes < 6 {
            return Err(Error::invalid(
                offset,
                "WMF record size is smaller than its header",
            ));
        }
        let end = offset
            .checked_add(size_bytes as u64)
            .ok_or_else(|| Error::invalid(offset, "WMF record size overflows"))?;
        if end > file_len {
            return Err(Error::invalid(
                offset,
                "WMF record extends past end of file",
            ));
        }
        let data = reader.read_vec(size_bytes as usize - 6)?;
        Ok(Self { function, data })
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        let size_bytes = self.data.len().checked_add(6).ok_or_else(|| {
            Error::invalid(writer.position().unwrap_or(0), "WMF record is too large")
        })?;
        if size_bytes % 2 != 0 {
            return Err(Error::invalid(
                writer.position()?,
                "WMF record data must include WORD alignment padding",
            ));
        }
        let size_words = size_bytes / 2;
        if size_words > u32::MAX as usize {
            return Err(Error::invalid(
                writer.position()?,
                "WMF record size exceeds u32::MAX WORDs",
            ));
        }
        writer.write_u32(size_words as u32)?;
        writer.write_u16(self.function)?;
        writer.write_all(&self.data)
    }
}

impl SdkSize for WmfRecord {
    fn sdk_size(&self) -> u64 {
        6 + self.data.len() as u64
    }
}

pub fn looks_like_wmf(bytes: &[u8]) -> bool {
    let offset = if has_placeable_header(bytes) {
        PLACEABLE_HEADER_SIZE
    } else {
        0
    };
    if bytes.len() < offset + WMF_HEADER_SIZE {
        return false;
    }
    let metafile_type = u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("slice length checked"),
    );
    let header_size = u16::from_le_bytes(
        bytes[offset + 2..offset + 4]
            .try_into()
            .expect("slice length checked"),
    );
    let version = u16::from_le_bytes(
        bytes[offset + 4..offset + 6]
            .try_into()
            .expect("slice length checked"),
    );
    matches!(metafile_type, 1 | 2) && header_size == 9 && matches!(version, 0x0100 | 0x0300)
}

fn has_placeable_header(bytes: &[u8]) -> bool {
    bytes.len() >= PLACEABLE_HEADER_SIZE
        && u32::from_le_bytes(bytes[0..4].try_into().expect("slice length checked"))
            == PLACEABLE_KEY
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_wmf() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&9u16.to_le_bytes());
        bytes.extend_from_slice(&0x0300u16.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&META_EOF.to_le_bytes());
        bytes
    }

    #[test]
    fn wmf_roundtrip_preserves_bytes() {
        let bytes = minimal_wmf();
        let metafile = WmfMetafile::from_bytes(&bytes).unwrap();
        assert_eq!(metafile.records.len(), 1);
        assert_eq!(metafile.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn detects_wmf_header() {
        assert!(looks_like_wmf(&minimal_wmf()));
    }

    #[test]
    fn maps_wmf_record_function_enum() {
        let record = WmfRecord::new(META_EOF, Vec::new());
        assert_eq!(record.function_kind(), Some(WmfRecordFunction::Eof));
        assert_eq!(WmfRecordFunction::ExtTextOut.raw(), 0x0A32);
    }
}
