use std::{path::PathBuf, sync::{mpsc::{channel, Sender}, Arc}};

use errors::{new_custom_error, GenericError};
use thread_pool::ThreadPool;

use crate::{file_reader::FileReader, list_to_path, FileEntry};

pub enum ReaderState {
    Def(FileEntry),
    Reader(Arc<FileReader>),
    Closed
}

pub enum ReaderResult {
    FirstInstance(Arc<FileReader>),
    Instance(Arc<FileReader>),
    NoReader
}

pub enum FileReaderMessage {
    GetReader(u32, Sender<ReaderResult>),
    ReaderFinished(u32)
}

pub struct FileReaderManager {
    channel: Sender<FileReaderMessage>
}

impl FileReaderManager {
    pub fn new(
        root: PathBuf,
        files: &Vec<FileEntry>,
        max_live_readers: u8) -> Self {

        let (message_sender, receiver) = channel();
        let messege_sender_clone = message_sender.clone();

        let mut files: Vec<ReaderState> = files.iter().map(|f| {
            ReaderState::Def(f.clone())
        }).collect();

        let pool = ThreadPool::new(max_live_readers + 1);
        let pool_clone = pool.clone();

        let root_clone = root.clone();
        pool.execute(move || -> Result<(), GenericError> {
            let root = root_clone;
            let message_sender = messege_sender_clone;

            loop {
                let mess: FileReaderMessage = receiver.recv()?;

                match mess {
                    FileReaderMessage::GetReader(id, sender) => {
                        let state = &files[id as usize];
                        match state {
                            ReaderState::Closed => {
                                sender.send(ReaderResult::NoReader)?;
                            }
                            ReaderState::Reader(reader) => {
                                let reader = Arc::clone(&reader);
                                sender.send(ReaderResult::Instance(reader))?;
                            }
                            ReaderState::Def(f) => {
                                let message_sender = message_sender.clone();
                                let file = list_to_path(&f.partial_path);
                                let name = file.to_str()
                                    .ok_or(new_custom_error("no file name"))?
                                    .to_owned();
                                let file = root.join(file);
                                let reader = FileReader::new(id, name, file, f.size, &pool_clone, message_sender);
                                let reader = Arc::new(reader);
                                files[id as usize] = ReaderState::Reader(Arc::clone(&reader));
                                sender.send(ReaderResult::FirstInstance(reader))?;
                            }
                        }
                    }

                    FileReaderMessage::ReaderFinished(id) => {
                        files[id as usize] = ReaderState::Closed;
                    }
                }
            }
        });

        FileReaderManager{
            channel: message_sender
        }
    }

    pub fn get_reader(&self, id: u32) -> ReaderResult {
        let (sender, receiver) = channel();
        self.channel.send(FileReaderMessage::GetReader(id, sender)).unwrap(); 
        let res = receiver.recv().unwrap();
        res
    }
}

