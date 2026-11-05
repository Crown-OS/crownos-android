use crate::WireError;

/// Appends little-endian fields to a payload.
#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u32(&mut self, value: u32) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn i32(&mut self, value: i32) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn bool(&mut self, value: bool) -> &mut Self {
        self.u32(u32::from(value))
    }

    pub fn str(&mut self, value: &str) -> Result<&mut Self, WireError> {
        let len = u16::try_from(value.len()).map_err(|_| WireError::StringTooLong)?;
        self.0.extend_from_slice(&len.to_le_bytes());
        self.0.extend_from_slice(value.as_bytes());
        Ok(self)
    }

    pub fn u32_list(&mut self, values: &[u32]) -> Result<&mut Self, WireError> {
        let count = u16::try_from(values.len()).map_err(|_| WireError::ListTooLong)?;
        self.0.extend_from_slice(&count.to_le_bytes());
        values.iter().for_each(|&value| {
            self.u32(value);
        });
        Ok(self)
    }
}

/// Reads little-endian fields from a payload. Every getter returns `None`
/// when the payload is too short or a string is not UTF-8.
pub struct Reader<'a>(pub &'a [u8]);

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Option<&[u8]> {
        let (head, rest) = self.0.split_at_checked(count)?;
        self.0 = rest;
        Some(head)
    }

    pub fn u16(&mut self) -> Option<u16> {
        self.take(2)?.try_into().ok().map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Option<u32> {
        self.take(4)?.try_into().ok().map(u32::from_le_bytes)
    }

    pub fn i32(&mut self) -> Option<i32> {
        self.take(4)?.try_into().ok().map(i32::from_le_bytes)
    }

    pub fn bool(&mut self) -> Option<bool> {
        match self.u32()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    pub fn str(&mut self) -> Option<String> {
        let len = usize::from(self.u16()?);
        String::from_utf8(self.take(len)?.to_vec()).ok()
    }

    pub fn u32_list(&mut self) -> Option<Vec<u32>> {
        let count = usize::from(self.u16()?);
        (0..count).map(|_| self.u32()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
