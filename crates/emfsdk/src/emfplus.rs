use emfsdk_derive::{SdkEnum, SdkObject};

use crate::common::{Error, Reader, Result, SdkEnumValue, Writer};

pub const EMFPLUS_METAFILE_SIGNATURE: u32 = 0xDBC01;

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u16")]
pub enum EmfPlusRecordType {
    Header = 0x4001,
    Eof = 0x4002,
    Comment = 0x4003,
    GetDc = 0x4004,
    MultiFormatStart = 0x4005,
    MultiFormatSection = 0x4006,
    MultiFormatEnd = 0x4007,
    Object = 0x4008,
    Clear = 0x4009,
    FillRects = 0x400A,
    DrawRects = 0x400B,
    FillPolygon = 0x400C,
    DrawLines = 0x400D,
    FillEllipse = 0x400E,
    DrawEllipse = 0x400F,
    FillPie = 0x4010,
    DrawPie = 0x4011,
    DrawArc = 0x4012,
    FillRegion = 0x4013,
    FillPath = 0x4014,
    DrawPath = 0x4015,
    FillClosedCurve = 0x4016,
    DrawClosedCurve = 0x4017,
    DrawCurve = 0x4018,
    DrawBeziers = 0x4019,
    DrawImage = 0x401A,
    DrawImagePoints = 0x401B,
    DrawString = 0x401C,
    SetRenderingOrigin = 0x401D,
    SetAntiAliasMode = 0x401E,
    SetTextRenderingHint = 0x401F,
    SetTextContrast = 0x4020,
    SetInterpolationMode = 0x4021,
    SetPixelOffsetMode = 0x4022,
    SetCompositingMode = 0x4023,
    SetCompositingQuality = 0x4024,
    Save = 0x4025,
    Restore = 0x4026,
    BeginContainer = 0x4027,
    BeginContainerNoParams = 0x4028,
    EndContainer = 0x4029,
    SetWorldTransform = 0x402A,
    ResetWorldTransform = 0x402B,
    MultiplyWorldTransform = 0x402C,
    TranslateWorldTransform = 0x402D,
    ScaleWorldTransform = 0x402E,
    RotateWorldTransform = 0x402F,
    SetPageTransform = 0x4030,
    ResetClip = 0x4031,
    SetClipRect = 0x4032,
    SetClipRegion = 0x4033,
    SetClipPath = 0x4034,
    OffsetClip = 0x4035,
    DrawDriverString = 0x4036,
    StrokeFillPath = 0x4037,
    SerializableObject = 0x4038,
    SetTsGraphics = 0x4039,
    SetTsClip = 0x403A,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u16")]
pub enum EmfPlusGraphicsVersionValue {
    Version1 = 0x0001,
    Version1_1 = 0x0002,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusGraphicsVersion {
    pub value: u32,
}

impl EmfPlusGraphicsVersion {
    pub fn metafile_signature(&self) -> u32 {
        self.value >> 12
    }

    pub fn graphics_version_raw(&self) -> u16 {
        (self.value & 0x0FFF) as u16
    }

    pub fn graphics_version(&self) -> Option<EmfPlusGraphicsVersionValue> {
        EmfPlusGraphicsVersionValue::from_raw(self.graphics_version_raw())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
#[sdk(format = "emfplus", record_type = 0x4001)]
pub struct EmfPlusHeaderData {
    pub graphics_version: EmfPlusGraphicsVersion,
    pub emf_plus_flags: u32,
    pub logical_dpi_x: u32,
    pub logical_dpi_y: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmfPlusRecord {
    pub record_type: u16,
    pub flags: u16,
    pub data: Vec<u8>,
    pub padding: Vec<u8>,
}

impl EmfPlusRecord {
    pub fn record_kind(&self) -> Option<EmfPlusRecordType> {
        EmfPlusRecordType::from_raw(self.record_type)
    }

    pub fn read_from<R: std::io::Read + std::io::Seek>(
        reader: &mut Reader<R>,
        stream_len: u64,
    ) -> Result<Self> {
        let offset = reader.position()?;
        let record_type = reader.read_u16()?;
        let flags = reader.read_u16()?;
        let size = reader.read_u32()?;
        let data_size = reader.read_u32()?;

        if size < 12 {
            return Err(Error::invalid(
                offset,
                "EMF+ record size is smaller than its header",
            ));
        }
        if data_size > size - 12 {
            return Err(Error::invalid(
                offset,
                "EMF+ record data size exceeds record size",
            ));
        }
        let end = offset
            .checked_add(size as u64)
            .ok_or_else(|| Error::invalid(offset, "EMF+ record size overflows"))?;
        if end > stream_len {
            return Err(Error::invalid(
                offset,
                "EMF+ record extends past end of stream",
            ));
        }

        let data = reader.read_vec(data_size as usize)?;
        let padding = reader.read_vec(size as usize - 12 - data_size as usize)?;

        Ok(Self {
            record_type,
            flags,
            data,
            padding,
        })
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        let payload_size = self
            .data
            .len()
            .checked_add(self.padding.len())
            .ok_or_else(|| {
                Error::invalid(writer.position().unwrap_or(0), "EMF+ record is too large")
            })?;
        let size = payload_size.checked_add(12).ok_or_else(|| {
            Error::invalid(writer.position().unwrap_or(0), "EMF+ record is too large")
        })?;
        if size > u32::MAX as usize {
            return Err(Error::invalid(
                writer.position()?,
                "EMF+ record size exceeds u32::MAX",
            ));
        }

        writer.write_u16(self.record_type)?;
        writer.write_u16(self.flags)?;
        writer.write_u32(size as u32)?;
        writer.write_u32(self.data.len() as u32)?;
        writer.write_all(&self.data)?;
        writer.write_all(&self.padding)
    }
}

pub fn read_records(bytes: &[u8]) -> Result<Vec<EmfPlusRecord>> {
    let mut reader = Reader::new(std::io::Cursor::new(bytes));
    let mut records = Vec::new();
    while reader.position()? < bytes.len() as u64 {
        records.push(EmfPlusRecord::read_from(&mut reader, bytes.len() as u64)?);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{SdkRead, SdkSize, SdkWrite};

    #[test]
    fn emf_plus_record_roundtrip_preserves_padding() {
        let bytes = [
            0x01, 0x40, // Type
            0x00, 0x00, // Flags
            0x10, 0x00, 0x00, 0x00, // Size
            0x02, 0x00, 0x00, 0x00, // DataSize
            0xAA, 0xBB, // Data
            0xCC, 0xDD, // Padding
        ];
        let records = read_records(&bytes).unwrap();
        assert_eq!(records[0].data, [0xAA, 0xBB]);
        assert_eq!(records[0].padding, [0xCC, 0xDD]);

        let mut writer = Writer::new(std::io::Cursor::new(Vec::new()));
        records[0].write_to(&mut writer).unwrap();
        assert_eq!(writer.into_inner().into_inner(), bytes);
        assert_eq!(records[0].record_kind(), Some(EmfPlusRecordType::Header));
    }

    #[test]
    fn derived_emf_plus_header_data_roundtrips() {
        let header = EmfPlusHeaderData {
            graphics_version: EmfPlusGraphicsVersion {
                value: (EMFPLUS_METAFILE_SIGNATURE << 12) | 0x0002,
            },
            emf_plus_flags: 1,
            logical_dpi_x: 96,
            logical_dpi_y: 96,
        };

        assert_eq!(
            header.graphics_version.metafile_signature(),
            EMFPLUS_METAFILE_SIGNATURE
        );
        assert_eq!(
            header.graphics_version.graphics_version(),
            Some(EmfPlusGraphicsVersionValue::Version1_1)
        );
        assert_eq!(header.sdk_size(), 16);

        let mut writer = Writer::new(std::io::Cursor::new(Vec::new()));
        header.write_to(&mut writer).unwrap();
        let bytes = writer.into_inner().into_inner();

        let mut reader = Reader::new(std::io::Cursor::new(bytes));
        let parsed = EmfPlusHeaderData::read_from(&mut reader).unwrap();
        assert_eq!(parsed, header);
    }
}
