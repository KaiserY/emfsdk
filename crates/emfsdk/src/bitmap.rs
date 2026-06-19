use emfsdk_derive::{SdkEnum, SdkObject};

use crate::common::{Error, Reader, Result, SdkEnumValue, SdkRead, SdkSize, SdkWrite, Writer};

pub const BITMAP_CORE_HEADER_SIZE: u32 = 12;
pub const BITMAP_INFO_HEADER_SIZE: u32 = 40;
pub const BITMAP_V4_HEADER_SIZE: u32 = 108;
pub const BITMAP_V5_HEADER_SIZE: u32 = 124;

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u32")]
pub enum DibColorUsage {
    RgbColors = 0x0000,
    PalColors = 0x0001,
    PalIndices = 0x0002,
}

impl DibColorUsage {
    pub fn from_wmf_raw(value: u16) -> Option<Self> {
        Self::from_raw(u32::from(value))
    }

    pub fn wmf_raw(self) -> u16 {
        self.raw() as u16
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u32")]
pub enum BitmapCompression {
    Rgb = 0x0000,
    Rle8 = 0x0001,
    Rle4 = 0x0002,
    Bitfields = 0x0003,
    Jpeg = 0x0004,
    Png = 0x0005,
    Cmyk = 0x000B,
    CmykRle8 = 0x000C,
    CmykRle4 = 0x000D,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddedBitmapFormat {
    Jpeg,
    Png,
}

impl EmbeddedBitmapFormat {
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}

impl BitmapCompression {
    pub fn embedded_format(self) -> Option<EmbeddedBitmapFormat> {
        match self {
            Self::Jpeg => Some(EmbeddedBitmapFormat::Jpeg),
            Self::Png => Some(EmbeddedBitmapFormat::Png),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u16")]
pub enum BitmapBitCount {
    Undefined = 0x0000,
    One = 0x0001,
    Four = 0x0004,
    Eight = 0x0008,
    Sixteen = 0x0010,
    TwentyFour = 0x0018,
    ThirtyTwo = 0x0020,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct BitmapCoreHeader {
    pub header_size: u32,
    pub width: u16,
    pub height: u16,
    pub planes: u16,
    pub bit_count: u16,
}

impl BitmapCoreHeader {
    pub fn bit_count_kind(&self) -> Option<BitmapBitCount> {
        BitmapBitCount::from_raw(self.bit_count)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct BitmapInfoHeader {
    pub header_size: u32,
    pub width: i32,
    pub height: i32,
    pub planes: u16,
    pub bit_count: u16,
    pub compression: u32,
    pub image_size: u32,
    pub x_pels_per_meter: i32,
    pub y_pels_per_meter: i32,
    pub color_used: u32,
    pub color_important: u32,
}

impl BitmapInfoHeader {
    pub fn compression_kind(&self) -> Option<BitmapCompression> {
        BitmapCompression::from_raw(self.compression)
    }

    pub fn bit_count_kind(&self) -> Option<BitmapBitCount> {
        BitmapBitCount::from_raw(self.bit_count)
    }

    pub fn is_top_down(&self) -> bool {
        self.height < 0
    }

    pub fn height_abs(&self) -> u32 {
        self.height.unsigned_abs()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DibHeader {
    Core(BitmapCoreHeader),
    Info {
        base: BitmapInfoHeader,
        extension: Vec<u8>,
    },
}

impl DibHeader {
    pub fn header_size(&self) -> u32 {
        match self {
            Self::Core(value) => value.header_size,
            Self::Info { base, .. } => base.header_size,
        }
    }

    pub fn width(&self) -> i32 {
        match self {
            Self::Core(value) => i32::from(value.width),
            Self::Info { base, .. } => base.width,
        }
    }

    pub fn height(&self) -> i32 {
        match self {
            Self::Core(value) => i32::from(value.height),
            Self::Info { base, .. } => base.height,
        }
    }

    pub fn bit_count(&self) -> u16 {
        match self {
            Self::Core(value) => value.bit_count,
            Self::Info { base, .. } => base.bit_count,
        }
    }

    pub fn bit_count_kind(&self) -> Option<BitmapBitCount> {
        BitmapBitCount::from_raw(self.bit_count())
    }

    pub fn compression_kind(&self) -> Option<BitmapCompression> {
        match self {
            Self::Core(_) => Some(BitmapCompression::Rgb),
            Self::Info { base, .. } => base.compression_kind(),
        }
    }

    pub fn is_top_down(&self) -> bool {
        matches!(self, Self::Info { base, .. } if base.is_top_down())
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        match self {
            Self::Core(value) => value.write_to(writer),
            Self::Info { base, extension } => {
                base.write_to(writer)?;
                writer.write_all(extension)
            }
        }
    }
}

impl SdkSize for DibHeader {
    fn sdk_size(&self) -> u64 {
        match self {
            Self::Core(value) => value.sdk_size(),
            Self::Info { base, extension } => base.sdk_size() + extension.len() as u64,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DibBitmapInfo {
    pub header: DibHeader,
    pub color_table: Vec<u8>,
}

impl DibBitmapInfo {
    pub fn read_from_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 4 {
            return Err(Error::invalid(
                0,
                "DIB bitmap info is smaller than HeaderSize",
            ));
        }

        let mut reader = Reader::new(std::io::Cursor::new(bytes));
        let header_size = reader.read_u32()?;
        reader.seek(0)?;

        let header = match header_size {
            BITMAP_CORE_HEADER_SIZE => {
                if bytes.len() < BITMAP_CORE_HEADER_SIZE as usize {
                    return Err(Error::invalid(0, "BitmapCoreHeader is truncated"));
                }
                DibHeader::Core(BitmapCoreHeader::read_from(&mut reader)?)
            }
            size if size >= BITMAP_INFO_HEADER_SIZE => {
                if bytes.len() < size as usize {
                    return Err(Error::invalid(0, "BitmapInfoHeader extension is truncated"));
                }
                let base = BitmapInfoHeader::read_from(&mut reader)?;
                let extension_size = size as usize - BITMAP_INFO_HEADER_SIZE as usize;
                let extension = reader.read_vec(extension_size)?;
                DibHeader::Info { base, extension }
            }
            _ => {
                return Err(Error::invalid(
                    0,
                    format!("unsupported DIB header size {header_size}"),
                ));
            }
        };

        let header_size = header.header_size() as usize;
        Ok(Self {
            header,
            color_table: bytes[header_size..].to_vec(),
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(std::io::Cursor::new(Vec::with_capacity(
            self.header.sdk_size() as usize + self.color_table.len(),
        )));
        self.header.write_to(&mut writer)?;
        writer.write_all(&self.color_table)?;
        Ok(writer.into_inner().into_inner())
    }

    pub fn compression_kind(&self) -> Option<BitmapCompression> {
        self.header.compression_kind()
    }

    pub fn embedded_format(&self) -> Option<EmbeddedBitmapFormat> {
        self.compression_kind()?.embedded_format()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceIndependentBitmap {
    pub info: DibBitmapInfo,
    pub bits: Vec<u8>,
}

impl DeviceIndependentBitmap {
    pub fn from_parts(bitmap_info: &[u8], bitmap_bits: &[u8]) -> Result<Self> {
        Ok(Self {
            info: DibBitmapInfo::read_from_slice(bitmap_info)?,
            bits: bitmap_bits.to_vec(),
        })
    }

    pub fn to_packed_bytes(&self) -> Result<Vec<u8>> {
        let info = self.info.to_bytes()?;
        let mut bytes = Vec::with_capacity(info.len() + self.bits.len());
        bytes.extend_from_slice(&info);
        bytes.extend_from_slice(&self.bits);
        Ok(bytes)
    }

    pub fn embedded_format(&self) -> Option<EmbeddedBitmapFormat> {
        self.info.embedded_format()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_info_header_parses_typed_fields() {
        let bytes = [
            40, 0, 0, 0, // HeaderSize
            3, 0, 0, 0, // Width
            0xFC, 0xFF, 0xFF, 0xFF, // Height = -4
            1, 0, // Planes
            32, 0, // BitCount
            5, 0, 0, 0, // BI_PNG
            9, 0, 0, 0, // ImageSize
            0, 0, 0, 0, // XPelsPerMeter
            0, 0, 0, 0, // YPelsPerMeter
            0, 0, 0, 0, // ColorUsed
            0, 0, 0, 0, // ColorImportant
        ];

        let info = DibBitmapInfo::read_from_slice(&bytes).unwrap();
        let DibHeader::Info { base, extension } = &info.header else {
            unreachable!();
        };
        assert_eq!(base.width, 3);
        assert_eq!(base.height, -4);
        assert_eq!(base.bit_count_kind(), Some(BitmapBitCount::ThirtyTwo));
        assert_eq!(base.compression_kind(), Some(BitmapCompression::Png));
        assert!(base.is_top_down());
        assert!(extension.is_empty());
        assert_eq!(info.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn dib_info_preserves_header_extension_and_color_table() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&BITMAP_V4_HEADER_SIZE.to_le_bytes());
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(&(BitmapCompression::Bitfields.raw()).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0xAB, 68));
        bytes.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);

        let info = DibBitmapInfo::read_from_slice(&bytes).unwrap();
        let DibHeader::Info { base, extension } = &info.header else {
            unreachable!();
        };
        assert_eq!(base.header_size, BITMAP_V4_HEADER_SIZE);
        assert_eq!(extension.len(), 68);
        assert_eq!(info.color_table, [0x01, 0x02, 0x03, 0x04]);
        assert_eq!(info.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn device_independent_bitmap_detects_embedded_png() {
        let bitmap_info = [
            40, 0, 0, 0, // HeaderSize
            5, 0, 0, 0, // Width
            6, 0, 0, 0, // Height
            1, 0, // Planes
            0, 0, // BitCount
            5, 0, 0, 0, // BI_PNG
            4, 0, 0, 0, // ImageSize
            0, 0, 0, 0, // XPelsPerMeter
            0, 0, 0, 0, // YPelsPerMeter
            0, 0, 0, 0, // ColorUsed
            0, 0, 0, 0, // ColorImportant
        ];
        let bitmap_bits = [0x89, b'P', b'N', b'G'];

        let dib = DeviceIndependentBitmap::from_parts(&bitmap_info, &bitmap_bits).unwrap();
        assert_eq!(dib.embedded_format(), Some(EmbeddedBitmapFormat::Png));
        assert_eq!(dib.embedded_format().unwrap().content_type(), "image/png");
        let packed = dib.to_packed_bytes().unwrap();
        assert_eq!(&packed[..bitmap_info.len()], bitmap_info);
        assert_eq!(&packed[bitmap_info.len()..], bitmap_bits);
    }
}
