use std::{io::Read, sync::{mpsc::{channel, Receiver, Sender}, Mutex}};

use errors::GenericError;

use crate::{file_chunk::{FileChunk, FILE_CHINK_MAX_SIZE}, file_reader_manager::FileReaderMessage};

pub struct FileReader {
    id: u32,
    pub chunk_receiver: Mutex<Receiver<Option<FileChunk>>>,
    chunk_sender: Sender<Option<FileChunk>>,
    slot_sender: Sender<()>
}

static MAX_CHUNKS: u8 = 10;

impl FileReader {
    pub fn new(
        id: u32,
        file: std::path::PathBuf,
        pool: &thread_pool::ThreadPool,
        finish_channel: Sender<FileReaderMessage>) -> FileReader {
        let (slot_sender, slot_receiver) = channel::<()>();
        let (chunk_sender, chunk_receiver) = channel();

        let chunk_sender_clone = chunk_sender.clone();

        pool.execute(move || -> Result<(), GenericError> {
            let chunk_sender = chunk_sender_clone;

            let mut file = std::fs::File::open(&file)?;
            let meta = file.metadata()?;
            let size = meta.len();

            let mut read = 0;

            let mut buf: Vec<u8> = vec![0; FILE_CHINK_MAX_SIZE];
            while read < size {
                slot_receiver.recv()?;

                file.read(&mut buf)?;
                let chunk = FileChunk::from_bytes(&buf);
                read += chunk.size;
                chunk_sender.send(Some(chunk))?;
            }
            chunk_sender.send(None)?;

            finish_channel.send(FileReaderMessage::ReaderFinished(id))?;

            Ok(())
        });

        for _ in 0..MAX_CHUNKS {
            slot_sender.send(()).unwrap();
        }

        FileReader {
            id,
            chunk_receiver: Mutex::new(chunk_receiver),
            chunk_sender,
            slot_sender
        }
    }

    pub fn get_chunk(&self) -> Option<FileChunk> {
        let chunk = { 
            let receiver = &*self.chunk_receiver.lock().unwrap();
            receiver.recv().unwrap()
        };
        match chunk {
            Some(chunk) => {
                let _ = self.slot_sender.send(());
                Some(chunk)
            }
            None => {
                self.chunk_sender.send(None).unwrap();
                None
            }
        }
    }
}
