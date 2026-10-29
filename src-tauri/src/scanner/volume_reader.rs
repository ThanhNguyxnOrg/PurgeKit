use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const VOLUME_BLOCK_SIZE: u64 = 64 * 1024;

/// Volume handles require sector-aligned reads. `binrw` (used by the ntfs
/// crate for the boot sector) performs tiny 3–8 byte reads, which Windows
/// rejects with ERROR_INVALID_PARAMETER. Serve arbitrary reads from an
/// aligned 64 KB block cache instead.
pub(crate) struct AlignedVolumeReader {
    file: File,
    position: u64,
    block_start: u64,
    block: Vec<u8>,
}

impl AlignedVolumeReader {
    pub(crate) fn new(file: File) -> Self {
        Self {
            file,
            position: 0,
            block_start: u64::MAX,
            block: Vec::new(),
        }
    }
}

impl Read for AlignedVolumeReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut written = 0usize;
        while written < buf.len() {
            let block_start = (self.position / VOLUME_BLOCK_SIZE) * VOLUME_BLOCK_SIZE;
            if self.block_start != block_start {
                self.file.seek(SeekFrom::Start(block_start))?;
                self.block.resize(VOLUME_BLOCK_SIZE as usize, 0);
                let read = self.file.read(&mut self.block)?;
                self.block.truncate(read);
                self.block_start = block_start;
                if read == 0 {
                    break;
                }
            }
            let offset = (self.position - block_start) as usize;
            if offset >= self.block.len() {
                break;
            }
            let count = (buf.len() - written).min(self.block.len() - offset);
            buf[written..written + count].copy_from_slice(&self.block[offset..offset + count]);
            written += count;
            self.position += count as u64;
        }
        Ok(written)
    }
}

impl Seek for AlignedVolumeReader {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let new_position = match position {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::Current(offset) => self.position as i128 + offset as i128,
            SeekFrom::End(offset) => {
                let length = self.file.metadata()?.len() as i128;
                length + offset as i128
            }
        };
        if new_position < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "negative seek position",
            ));
        }
        self.position = new_position as u64;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn serves_small_unaligned_reads() {
        let path = std::env::temp_dir().join(format!(
            "purgekit_aligned_volume_{}.bin",
            std::process::id()
        ));
        let data: Vec<u8> = (0..200_000).map(|index| (index % 251) as u8).collect();
        fs::write(&path, &data).unwrap();
        let file = File::open(&path).unwrap();
        let mut reader = AlignedVolumeReader::new(file);

        let mut small = [0u8; 3];
        reader.read_exact(&mut small).unwrap();
        assert_eq!(&small, &data[0..3]);

        reader.seek(SeekFrom::Start(100)).unwrap();
        let mut block = vec![0u8; 1024];
        reader.read_exact(&mut block).unwrap();
        assert_eq!(&block, &data[100..1124]);

        reader.seek(SeekFrom::Start(65530)).unwrap();
        let mut across = vec![0u8; 20];
        reader.read_exact(&mut across).unwrap();
        assert_eq!(&across, &data[65530..65550]);

        let _ = fs::remove_file(&path);
    }
}
