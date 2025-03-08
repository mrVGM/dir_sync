use std::{path::PathBuf, sync::{mpsc::{channel, Receiver, Sender}, Arc}};

use errors::GenericError;
use thread_pool::ThreadPool;

use crate::{file_reader::FileReader, list_to_path, FileEntry};

enum ReaderState {
    Def(FileEntry),
    Reader(Arc<FileReader>),
    Closed
}

pub enum FileReaderMessage {
    GetReader(u32, Sender<Option<Arc<FileReader>>>),
    ReaderFinished(u32)
}

pub struct FileReaderManager {
    root: PathBuf
}

impl FileReaderManager {
    pub fn new(
        root: PathBuf,
        files: &Vec<FileEntry>,
        pool: ThreadPool) -> Self {

        let (message_sender, receiver) = channel();

        let mut files: Vec<ReaderState> = files.iter().map(|f| {
            ReaderState::Def(f.clone())
        }).collect();

        let pool_clone = pool.clone();
        pool.execute(move || -> Result<(), GenericError> {

            loop {
                let mess: FileReaderMessage = receiver.recv()?;

                match mess {
                    FileReaderMessage::GetReader(id, sender) => {
                        let state = &files[id as usize];
                        match state {
                            ReaderState::Closed => {
                                sender.send(None)?;
                            }
                            ReaderState::Reader(reader) => {
                                let reader = Arc::clone(&reader);
                                sender.send(Some(reader))?;
                            }
                            ReaderState::Def(f) => {
                                let message_sender = message_sender.clone();
                                let file = list_to_path(&f.partial_path);
                                let reader = FileReader::new(id, file, &pool_clone, message_sender);
                                let reader = Arc::new(reader);
                                files[id as usize] = ReaderState::Reader(reader);
                            }
                        }
                    }

                    FileReaderMessage::ReaderFinished(id) => {
                        files[id as usize] = ReaderState::Closed;
                    }
                }
            }
        });

        FileReaderManager{ root }
    }
}
