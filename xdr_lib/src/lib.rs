use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq)]
pub struct DeserializeError;

impl std::error::Error for DeserializeError {}

impl std::fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Invalid input to deserialize method")
    }
}

pub type Result<T> = std::result::Result<T, DeserializeError>;

pub fn get_i32(dst: &mut i32, input: &mut &[u8]) -> Result<()> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<i32>());
    *input = rest;
    *dst = i32::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_u32(dst: &mut u32, input: &mut &[u8]) -> Result<()> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    *dst = u32::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_i64(dst: &mut i64, input: &mut &[u8]) -> Result<()> {
    if input.len() < 8 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<i64>());
    *input = rest;
    *dst = i64::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_u64(dst: &mut u64, input: &mut &[u8]) -> Result<()> {
    if input.len() < 8 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u64>());
    *input = rest;
    *dst = u64::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_bool(dst: &mut bool, input: &mut &[u8]) -> Result<()> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (bool_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    *dst = !matches!(u32::from_be_bytes(bool_bytes.try_into().unwrap()), 0);
    Ok(())
}

pub fn serialize_bool(src: &bool) -> [u8; 4] {
    match src {
        true => 1_u32.to_be_bytes(),
        false => 0_u32.to_be_bytes(),
    }
}

pub fn padded_4byte(len: usize) -> usize {
    (len + 3) & !(0b11usize)
}

pub fn encode_padding(offset: usize, buf: &mut [u8]) -> usize {
    let padded_offset: usize = padded_4byte(offset);
    buf[offset..padded_offset].fill(0u8);
    padded_offset
}

pub fn get_i32_infallible(input: &[u8]) -> i32 {
    let (int_bytes, _rest) = input.split_at(std::mem::size_of::<i32>());
    i32::from_be_bytes(int_bytes.try_into().unwrap())
}

pub fn get_u32_infallible(input: &[u8]) -> u32 {
    let (int_bytes, _rest) = input.split_at(std::mem::size_of::<u32>());
    u32::from_be_bytes(int_bytes.try_into().unwrap())
}

pub fn get_i64_infallible(input: &[u8]) -> i64 {
    let (int_bytes, _rest) = input.split_at(std::mem::size_of::<i64>());
    i64::from_be_bytes(int_bytes.try_into().unwrap())
}

pub fn get_u64_infallible(input: &[u8]) -> u64 {
    let (int_bytes, _rest) = input.split_at(std::mem::size_of::<u64>());
    u64::from_be_bytes(int_bytes.try_into().unwrap())
}

pub fn get_bool_infallible(input: &[u8]) -> bool {
    let (bool_bytes, _rest) = input.split_at(std::mem::size_of::<u32>());
    !matches!(u32::from_be_bytes(bool_bytes.try_into().unwrap()), 0)
}

pub fn geq_4byte_boundary(offset: usize) -> usize {
    (offset + 3) & !(0b11usize)
}

pub trait Reader<'a> {
    fn from_buf(buf: &'a [u8]) -> Result<Self>
    where
        Self: Sized;
    fn get_width(&self) -> Result<usize>;
}

#[derive(Default)]
pub struct ArrayIter<'a, T> {
    pub buf: &'a [u8],

    item_width: Option<usize>,
    count: usize,

    // DEFAULT INIT BELOW
    pub off: usize,
    pub i: usize,

    pub _marker: PhantomData<T>,
    err: bool,
}

#[derive(Default)]
pub struct LinkedListIter<'a, T> {
    pub buf: &'a [u8],

    item_width: Option<usize>,

    // DEFAULT INIT BELOW
    pub off: usize,
    pub i: usize,

    pub _marker: PhantomData<T>,
    err: bool,
}

macro_rules! impl_reader_for_numeric {
    ($(($t:ty, $func:ident, $size:expr)),*) => {
        $(
            impl<'a> Reader<'a> for $t {
                fn from_buf(buf: &'a [u8]) -> Result<Self> {
                    if buf.len() < $size {
                        Err(DeserializeError)
                    } else {
                        Ok($func(buf))
                    }
                }

                fn get_width(&self) -> Result<usize> {
                    Ok($size)
                }
            }
        )*
    };
}

impl_reader_for_numeric!(
    (i32, get_i32_infallible, 4),
    (u32, get_u32_infallible, 4),
    (i64, get_i64_infallible, 8),
    (u64, get_u64_infallible, 8),
    (bool, get_bool_infallible, 4)
);

impl<'a, T> Reader<'a> for Option<T>
where
    T: Reader<'a>,
{
    fn from_buf(buf: &'a [u8]) -> Result<Self> {
        if buf.len() < 4 {
            return Err(DeserializeError);
        }

        let has_optional = get_i32_infallible(buf);
        match has_optional {
            0 => Ok(None),
            _ => T::from_buf(&buf[4..]).map(|v| Some(v)),
        }
    }

    fn get_width(&self) -> Result<usize> {
        match self {
            Some(reader) => reader.get_width(),
            None => Ok(0), // Or return an Error, depending on your needs
        }
        .map(|v| v + 4usize)
    }
}

impl<'a, T> LinkedListIter<'a, T> {
    pub fn new(buf: &'a [u8], item_width: Option<usize>) -> Self {
        Self {
            buf,
            item_width,
            off: 0,
            i: 0,
            _marker: PhantomData,
            err: false,
        }
    }

    pub fn get_index(&self) -> usize {
        self.i
    }
}

impl<'a, T> Iterator for LinkedListIter<'a, T>
where
    T: Reader<'a>,
{
    type Item = Result<T>;

    fn next(&mut self) -> Option<Result<T>> {
        if self.err {
            return None;
        }

        if self.buf.len() < 4 + self.off {
            self.err = true;
            return Some(Err(DeserializeError));
        }

        let has_val = get_i32_infallible(&self.buf[self.off..]);

        self.off += 4;

        if has_val == 0 {
            return None;
        }

        let ret = if let Some(item_width) = self.item_width {
            T::from_buf(
                self.buf
                    .get(self.off..self.off + item_width)
                    .unwrap_or(&self.buf[self.off..]),
            )
        } else {
            T::from_buf(&self.buf[self.off..])
        };

        if let Ok(ret) = ret {
            self.off += ret.get_width().ok()?;
            self.i += 1;

            Some(Ok(ret))
        } else {
            self.err = true;
            Some(Err(DeserializeError))
        }
    }
}

impl<'a, T> ArrayIter<'a, T> {
    pub fn new(buf: &'a [u8], count: usize, item_width: Option<usize>) -> Self {
        Self {
            buf,
            item_width,
            count,
            off: 0,
            i: 0,
            _marker: PhantomData,
            err: false,
        }
    }

    pub fn get_index(&self) -> usize {
        self.i
    }

    pub fn get_count(&self) -> usize {
        self.count
    }
}

impl<'a, T> Iterator for ArrayIter<'a, T>
where
    T: Reader<'a>,
{
    type Item = Result<T>;

    fn next(&mut self) -> Option<Result<T>> {
        if self.err {
            return None;
        }

        if self.i == self.count {
            return None;
        }

        if self.off >= self.buf.len() || self.i >= self.count {
            self.err = true;
            return Some(Err(DeserializeError));
        }

        let ret = if let Some(item_width) = self.item_width {
            T::from_buf(&self.buf[self.off..self.off + item_width])
        } else {
            T::from_buf(&self.buf[self.off..])
        };

        if let Ok(ret) = ret {
            self.off += ret.get_width().ok()?;
            self.i += 1;

            Some(Ok(ret))
        } else {
            self.err = true;
            Some(Err(DeserializeError))
        }
    }
}
