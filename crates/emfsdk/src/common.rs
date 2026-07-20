use std::io::{Read, Seek, SeekFrom, Write};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
  #[error("invalid metafile data at offset {offset}: {message}")]
  InvalidData { offset: u64, message: String },
  #[error("unsupported metafile format")]
  UnsupportedFormat,
  #[error("string encoding error for {encoding}: {message}")]
  Encoding { encoding: String, message: String },
}

impl Error {
  pub fn invalid(offset: u64, message: impl Into<String>) -> Self {
    Self::InvalidData {
      offset,
      message: message.into(),
    }
  }

  pub fn encoding(encoding: impl Into<String>, message: impl Into<String>) -> Self {
    Self::Encoding {
      encoding: encoding.into(),
      message: message.into(),
    }
  }

  pub const fn offset(&self) -> Option<u64> {
    match self {
      Self::InvalidData { offset, .. } => Some(*offset),
      Self::Io(_) | Self::UnsupportedFormat | Self::Encoding { .. } => None,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
  Emf,
  Wmf,
}

pub trait SdkRead: Sized {
  fn read_from<R: Read + Seek>(reader: &mut Reader<R>) -> Result<Self>;
}

pub trait SdkWrite {
  fn write_to<W: Write + Seek>(&self, writer: &mut Writer<W>) -> Result<()>;
}

pub trait SdkSize {
  fn sdk_size(&self) -> u64;
}

pub trait SdkEnumValue: Copy + Sized {
  type Repr: Copy + PartialEq;

  fn from_raw(value: Self::Repr) -> Option<Self>;

  fn raw(self) -> Self::Repr;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownRecord {
  pub record_type: u32,
  pub data: Vec<u8>,
}

pub struct Reader<R> {
  inner: R,
}

impl<R: Read + Seek> Reader<R> {
  pub fn new(inner: R) -> Self {
    Self { inner }
  }

  pub fn into_inner(self) -> R {
    self.inner
  }

  pub fn position(&mut self) -> Result<u64> {
    Ok(self.inner.stream_position()?)
  }

  pub fn seek(&mut self, position: u64) -> Result<()> {
    self.inner.seek(SeekFrom::Start(position))?;
    Ok(())
  }

  pub fn skip(&mut self, len: u64) -> Result<()> {
    let current = self.position()?;
    let position = current
      .checked_add(len)
      .ok_or_else(|| Error::invalid(current, "reader position overflows"))?;
    self.seek(position)
  }

  pub fn read_u8(&mut self) -> Result<u8> {
    let mut buf = [0; 1];
    self.inner.read_exact(&mut buf)?;
    Ok(buf[0])
  }

  pub fn read_i8(&mut self) -> Result<i8> {
    Ok(self.read_u8()? as i8)
  }

  pub fn read_u16(&mut self) -> Result<u16> {
    let mut buf = [0; 2];
    self.inner.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
  }

  pub fn read_i16(&mut self) -> Result<i16> {
    Ok(self.read_u16()? as i16)
  }

  pub fn read_u32(&mut self) -> Result<u32> {
    let mut buf = [0; 4];
    self.inner.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
  }

  pub fn read_i32(&mut self) -> Result<i32> {
    Ok(self.read_u32()? as i32)
  }

  pub fn read_u64(&mut self) -> Result<u64> {
    let mut buf = [0; 8];
    self.inner.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
  }

  pub fn read_i64(&mut self) -> Result<i64> {
    Ok(self.read_u64()? as i64)
  }

  pub fn read_f32(&mut self) -> Result<f32> {
    Ok(f32::from_bits(self.read_u32()?))
  }

  pub fn read_f64(&mut self) -> Result<f64> {
    Ok(f64::from_bits(self.read_u64()?))
  }

  pub fn read_vec(&mut self, len: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0; len];
    self.inner.read_exact(&mut buf)?;
    Ok(buf)
  }

  pub fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
    let mut buf = [0; N];
    self.inner.read_exact(&mut buf)?;
    Ok(buf)
  }
}

pub struct Writer<W> {
  inner: W,
}

impl<W: Write + Seek> Writer<W> {
  pub fn new(inner: W) -> Self {
    Self { inner }
  }

  pub fn into_inner(self) -> W {
    self.inner
  }

  pub fn position(&mut self) -> Result<u64> {
    Ok(self.inner.stream_position()?)
  }

  pub fn write_u8(&mut self, value: u8) -> Result<()> {
    self.inner.write_all(&[value])?;
    Ok(())
  }

  pub fn write_i8(&mut self, value: i8) -> Result<()> {
    self.write_u8(value as u8)
  }

  pub fn write_u16(&mut self, value: u16) -> Result<()> {
    self.inner.write_all(&value.to_le_bytes())?;
    Ok(())
  }

  pub fn write_i16(&mut self, value: i16) -> Result<()> {
    self.inner.write_all(&value.to_le_bytes())?;
    Ok(())
  }

  pub fn write_u32(&mut self, value: u32) -> Result<()> {
    self.inner.write_all(&value.to_le_bytes())?;
    Ok(())
  }

  pub fn write_i32(&mut self, value: i32) -> Result<()> {
    self.inner.write_all(&value.to_le_bytes())?;
    Ok(())
  }

  pub fn write_u64(&mut self, value: u64) -> Result<()> {
    self.inner.write_all(&value.to_le_bytes())?;
    Ok(())
  }

  pub fn write_i64(&mut self, value: i64) -> Result<()> {
    self.inner.write_all(&value.to_le_bytes())?;
    Ok(())
  }

  pub fn write_f32(&mut self, value: f32) -> Result<()> {
    self.write_u32(value.to_bits())
  }

  pub fn write_f64(&mut self, value: f64) -> Result<()> {
    self.write_u64(value.to_bits())
  }

  pub fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
    self.inner.write_all(bytes)?;
    Ok(())
  }
}
