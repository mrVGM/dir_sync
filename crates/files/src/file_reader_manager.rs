use std::{path::PathBuf, sync::{mpsc::{channel, Receiver, Sender}, Arc}};

use errors::GenericError;
use thread_pool::ThreadPool;

use crate::{file_reader::FileReader, list_to_path, FileEntry};

pub enum ReaderState {
    Def(FileEntry),
    Reader(Arc<FileReader>),
    Closed
}

pub enum FileReaderMessage {
    GetReader(u32, Sender<Option<Arc<FileReader>>>),
    ReaderFinished(u32)
}

pub struct FileReaderManager {
    root: PathBuf,
    channel: Sender<FileReaderMessage>
}

impl FileReaderManager {
    pub fn new(
        root: PathBuf,
        files: &Vec<FileEntry>) -> Self {

        let (message_sender, receiver) = channel();
        let messege_sender_clone = message_sender.clone();

        let mut files: Vec<ReaderState> = files.iter().map(|f| {
            ReaderState::Def(f.clone())
        }).collect();

        let pool = ThreadPool::new(5);
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
                                sender.send(None)?;
                            }
                            ReaderState::Reader(reader) => {
                                let reader = Arc::clone(&reader);
                                sender.send(Some(reader))?;
                            }
                            ReaderState::Def(f) => {
                                let message_sender = message_sender.clone();
                                let file = list_to_path(&f.partial_path);
                                let file = root.join(file);
                                let reader = FileReader::new(id, file, &pool_clone, message_sender);
                                let reader = Arc::new(reader);
                                files[id as usize] = ReaderState::Reader(Arc::clone(&reader));
                                sender.send(Some(reader))?;
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
            root,
            channel: message_sender
        }
    }

    pub fn get_reader(&self, id: u32) -> Option<Arc<FileReader>> {
        dbg!(id);
        let (sender, receiver) = channel();
        self.channel.send(FileReaderMessage::GetReader(id, sender)).unwrap(); 
        receiver.recv().unwrap()
    }
}

