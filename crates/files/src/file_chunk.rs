static FILE_CHUNK_SIZE: usize = 8 * 1024 * 1024;
pub static FILE_CHUNK_MAX_SIZE: usize = 2 * size_of::<u64>() + FILE_CHUNK_SIZE;

#[derive(Debug)]
pub struct FileChunk {
    pub offset: u64,
    pub size: u64,
    pub data: Vec<u8>
}

impl FileChunk {
    pub fn new() -> Self {
        let mut data = Vec::with_capacity(FILE_CHUNK_SIZE);
        data.resize(FILE_CHUNK_SIZE, 0);

        FileChunk {
            offset: 0,
            size: 0,
            data
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let u64_size = size_of::<u64>();
        let u8_size = size_of::<u8>();
        let data_len = (self.size as usize) * size_of::<u8>(); 
        let len = 2 * u64_size + data_len;
        let mut res = Vec::with_capacity(len);
        res.resize(len, 0);

        {
            let offset_bytes = &mut res[0..u64_size];
            let offset = self.offset.to_be_bytes();
            offset_bytes.copy_from_slice(&offset);
        }

        {
            let size_bytes = &mut res[u64_size..2 * u64_size];
            let size = self.size.to_be_bytes();
            size_bytes.copy_from_slice(&size);
        }

        {
            let data_bytes = &mut res[2 * u64_size..];
            data_bytes.copy_from_slice(&self.data[..data_len]);
        }

        res
    }

    pub fn from_bytes(bytes: &[u8]) -> FileChunk {
        let u64_size = size_of::<u64>();
        let u8_size = size_of::<u8>();

        let offset = {
            let mut offset_bytes: [u8; 8] = [0; 8];
            offset_bytes.copy_from_slice(&bytes[0..u64_size]);
            u64::from_be_bytes(offset_bytes)
        };

        let size = {
            let mut size_bytes: [u8; 8] = [0; 8];
            size_bytes.copy_from_slice(&bytes[u64_size..2 * u64_size]);
            u64::from_be_bytes(size_bytes)
        };

        let mut data: Vec<u8> = vec![0; FILE_CHUNK_SIZE];
        {
            let tmp = &mut data[0..size as usize];
            tmp.copy_from_slice(&bytes[2 * u64_size.. 2 * u64_size + size as usize * u8_size]);
        }

        FileChunk {
            offset,
            size,
            data
        }
    }
}
