
use std::io::{self,Write,BufWriter,Seek,SeekFrom};
use std::fs::File;
use std::path::Path;
use std::marker::PhantomData;

use byteorder::{WriteBytesExt, LittleEndian};

use npy_data::NpyRecord;

const FILLER: &'static [u8] = &[42; 19];

/// Serialize into a file one row at a time. To serialize an iterator, use the
/// [`to_file`](fn.to_file.html) function.
pub struct OutFile<Row: NpyRecord, W: Write + Seek> {
    shape_pos: usize,
    len: usize,
    w: W,
    _t: PhantomData<Row>
}

impl<Row: NpyRecord> OutFile<Row, BufWriter<File>> {
    /// Open a file
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Self::new(BufWriter::new(File::create(path)?))
    }
}

impl<Row: NpyRecord, W: Write + Seek> OutFile<Row, W> {
    /// Create a new OutFile from a writer
    pub fn new(mut writer: W) -> io::Result<Self> {
        writer.write_all(&[0x93u8])?;
        writer.write_all(b"NUMPY")?;
        writer.write_all(&[0x01u8, 0x00])?;
        let mut header: Vec<u8> = vec![];
        header.extend(&b"{'descr': ["[..]);

        for (id, t) in Row::get_dtype() {

            if t.shape.len() == 0 {
                header.extend(format!("('{}', '{}'), ", id, t.ty).as_bytes());
            } else {
                let shape_str = t.shape.into_iter().fold(String::new(), |o,n| o + &format!("{},", n));
                header.extend(format!("('{}', '{}', ({})), ", id, t.ty, shape_str).as_bytes());
            }
        }

        header.extend(&b"], 'fortran_order': False, 'shape': ("[..]);
        let shape_pos = header.len() + 10;
        header.extend(FILLER);
        header.extend(&b",), }"[..]);

        let mut padding: Vec<u8> = vec![];
        padding.extend(&::std::iter::repeat(b' ').take(15 - ((header.len() + 10) % 16)).collect::<Vec<_>>());
        padding.extend(&[b'\n']);

        let len = header.len() + padding.len();
        assert! (len <= ::std::u16::MAX as usize);
        assert_eq!((len + 10) % 16, 0);

        writer.write_u16::<LittleEndian>(len as u16)?;
        writer.write_all(&header)?;
        // Padding to 8 bytes
        writer.write_all(&padding)?;

        Ok(OutFile {
            shape_pos: shape_pos,
            len: 0,
            w: writer,
            _t: PhantomData,
        })
    }

    /// Append a single `NpyRecord` instance to the file
    pub fn push(&mut self, row: &Row) -> io::Result<()> {
        self.len += 1;
        row.write(&mut self.w)
    }

    fn close_(&mut self) -> io::Result<()> {
        // Write the size to the header
        self.w.seek(SeekFrom::Start(self.shape_pos as u64))?;
        let length = format!("{}", self.len);
        self.w.write_all(length.as_bytes())?;
        self.w.write_all(&b",), }"[..])?;
        self.w.write_all(&::std::iter::repeat(b' ').take(FILLER.len() - length.len()).collect::<Vec<_>>())?;
        Ok(())
    }

    /// Finish writing the file by finalizing the header and closing the file.
    ///
    /// If omitted, the file will be closed on drop automatically, but it will panic on error.
    pub fn close(mut self) -> io::Result<()> {
        self.close_()
    }
}

impl<Row: NpyRecord, W: Write + Seek> Drop for OutFile<Row, W> {
    fn drop(&mut self) {
        let _ = self.close_(); // Ignore the errors
    }
}


// TODO: improve the interface to avoid unnecessary clones
/// Serialize an iterator over a struct to a NPY file
///
/// A single-statement alternative to saving row by row using the [`OutFile`](struct.OutFile.html).
pub fn to_file<'a, S, T, P>(filename: P, data: T) -> ::std::io::Result<()> where
        P: AsRef<Path>,
        S: NpyRecord + 'a,
        T: IntoIterator<Item=S> {

    let mut of = OutFile::open(filename)?;
    for row in data {
        of.push(&row)?;
    }
    of.close()
}

/// Serialize an iterator of a struct in NPY format to a buffer
pub fn write_all<'a, S, T, W>(writer: W, data: T) -> ::std::io::Result<()> where
        W: Write + Seek,
        S: NpyRecord + 'a,
        T: IntoIterator<Item=S> {

    let mut of = OutFile::new(writer)?;
    for row in data {
        of.push(&row)?;
    }
    of.close()
}
