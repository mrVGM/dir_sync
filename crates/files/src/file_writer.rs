use std::{io::Write, path::Path, sync::mpsc::{channel, Sender}};

use common::FileStreamMessage;
use errors::GenericError;
use thread_pool::ThreadPool;
use crate::file_chunk::FileChunk;

pub struct FileWriter {
    chunk_sender: Sender<FileChunk>,
}

impl FileWriter {
    pub fn new(
        id: u32,
        size: u64,
        path: &Path,
        finish_sender: Sender<FileStreamMessage>,
        pool: ThreadPool) ->
        Result<Self, GenericError> {

        let parent = path.parent();
        if let Some(parent) = parent {
            std::fs::create_dir_all(parent)?;
        }

        let mut writer = std::fs::File::create(path)?;

        let (chunk_sender, chunk_receiver) = channel::<FileChunk>();
        pool.execute(move || -> Result<(), GenericError> {
            let mut written = 0;
            let mut received = Vec::<FileChunk>::new();
            while written < size {
                let chunk = chunk_receiver.recv()?;
                let index = 'insert_index: {
                    for (i, c) in received.iter().enumerate() {
                        if chunk.offset < c.offset {
                            break 'insert_index i;
                        }
                    }
                    received.len()
                };
                received.insert(index, chunk);

                loop {
                    if let Some(chunk) = received.first() {
                        if chunk.offset > written {
                            break;
                        }
                    }
                    else {
                        break;
                    }

                    let front = received.remove(0);
                    writer.write(&front.data[..front.size as usize])?;
                    written += front.size;
                }
            }

            finish_sender.send(FileStreamMessage::Finish(id))?;
            Ok(())
        });

        Ok(FileWriter {
            chunk_sender,
        })
    }

    pub fn push_chunk(&self, chunk: FileChunk) -> Result<(), GenericError> {
        let _ = self.chunk_sender.send(chunk)?;
        Ok(())
    }
}

