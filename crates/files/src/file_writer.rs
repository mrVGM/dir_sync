use std::{io::Write, path::Path, sync::{mpsc::{channel, Receiver, Sender}, Arc}};

use common::FileStreamMessage;
use errors::{new_custom_error, GenericError};
use thread_pool::ThreadPool;
use crate::{file_chunk::{FileChunk, FILE_CHUNK_MAX_SIZE}, file_reader_manager::FileReaderMessage};

pub struct FileWriter {
    id: u32,
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

        // let mut writer = std::fs::File::create(path)?;

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
                    // writer.write(&front.data)?;
                    written += front.size;
                }
            }

            finish_sender.send(FileStreamMessage::Finish(id))?;
            Ok(())
        });

        Ok(FileWriter {
            id,
            chunk_sender,
        })
    }

    pub fn push_chunk(&self, chunk: FileChunk) -> Result<(), GenericError> {
        let res = self.chunk_sender.send(chunk);
        if let Err(_) = res {
            return Err(new_custom_error("the error"));
        }
        Ok(())
    }
}

