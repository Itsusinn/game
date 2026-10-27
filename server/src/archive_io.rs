//! Shared immutable mmap envelope. The file handle outlives all archived borrows.
use anyhow::{Context, Result};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

const HEADER: usize = 16; // Keep the rkyv payload aligned.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn write_new(path: &Path, magic: &[u8; 8], bytes: &[u8]) -> Result<()> {
    anyhow::ensure!(
        bytes.len() as u64 + HEADER as u64 <= MAX_BYTES,
        "archive too large"
    );
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let file = options.open(path)?;
    file.set_len((HEADER + bytes.len()) as u64)?;
    // Newly created exclusive file; no writer/map is retained by the caller.
    let mut map = unsafe { memmap2::MmapOptions::new().map_mut(&file)? };
    map[..8].copy_from_slice(magic);
    map[8..HEADER].copy_from_slice(&(bytes.len() as u64).to_le_bytes());
    map[HEADER..].copy_from_slice(bytes);
    map.flush()?;
    drop(map);
    file.sync_all()?;
    Ok(())
}

pub(crate) struct MappedArchive {
    map: memmap2::Mmap,
    // Windows share restrictions must remain active until all mapped references
    // are gone. Drop order releases the map before the file handle.
    _file: File,
}

impl MappedArchive {
    pub(crate) fn open(path: &Path, magic: &[u8; 8]) -> Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1); // read sharing only: deny writes and deletion
        }
        let file = options.open(path).context("open archive")?;
        let length = file.metadata()?.len();
        anyhow::ensure!(
            (HEADER as u64..=MAX_BYTES).contains(&length),
            "invalid archive length"
        );
        #[cfg(windows)]
        // The retained file handle prevents mutation/truncation while mapped.
        let map = unsafe { memmap2::MmapOptions::new().map(&file)? };
        #[cfg(not(windows))]
        let map = {
            use std::io::Read;
            let mut map = memmap2::MmapOptions::new()
                .len(length as usize)
                .map_anon()?;
            (&file).read_exact(&mut map)?;
            map.make_read_only()?
        };
        anyhow::ensure!(&map[..8] == magic, "invalid archive magic");
        let payload = u64::from_le_bytes(map[8..HEADER].try_into()?);
        anyhow::ensure!(payload == length - HEADER as u64, "truncated archive");
        Ok(Self { map, _file: file })
    }
    pub(crate) fn payload(&self) -> &[u8] {
        &self.map[HEADER..]
    }
}
