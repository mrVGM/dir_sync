use std::{io::Write, path::PathBuf, sync::mpsc::channel};

use errors::{new_custom_error, GenericError};
use files::{FileEntry, FileReaderManager};
use net::{JSONReader, TcpEndpoint};
use thread_pool::ThreadPool;

use crate::messages::{DSMessage, DSMessageType, DownloadFile, MessageFiles};

pub fn send_files(
    mut tcp_endpoint: impl TcpEndpoint,
    dir: PathBuf) -> Result<(), GenericError> {

    let mut main_stream = tcp_endpoint.wait_for_connection()?;
    let mut reader = {
        let main_stream_clone = main_stream.try_clone()?;
        JSONReader::new(main_stream_clone)
    };

    let json = reader.read_json()?;
    let message: DSMessage = serde_json::from_value(json)?;
    let files = files::get_files_in_dir(&dir)?;

    enum FileStreamState {
        NotStarted(FileEntry),
        Working(u8),
        Finished
    }

    let mut file_streams: Vec<FileStreamState> = files.iter()
        .map(|f| {
            match f.size {
                0 => FileStreamState::Finished,
                _ => FileStreamState::NotStarted(f.clone())
            }
        }).collect();

    let mut files_to_send = file_streams.iter()
        .filter(|f| {
            match f {
                FileStreamState::NotStarted(_) => true,
                _ => false
            }
        }).count();

    let manager = FileReaderManager::new(dir, &files);

    match message.message_type {
        DSMessageType::GetFileList => {
            let files = MessageFiles {
                files
            };

            let json = serde_json::to_string(&files)?;
            main_stream.write(json.as_bytes())?;
        }
    }


    enum FileStreamMessage {
        Start(u32),
        Finish(u32)
    }
    let (fs_sender, fs_receiver) = channel();

    let pool = ThreadPool::new(9);
    let pool_clone = pool.clone();
    pool.execute(move || -> Result<(), GenericError> {
        let pool = pool_clone;
        loop {
            let stream = tcp_endpoint.get_connection()?;

            let mut reader = { 
                let stream_clone = stream.try_clone()?;
                JSONReader::new(stream_clone)
            };
            let message = reader.read_json()?;

            let download: DownloadFile = serde_json::from_value(message)?;
            let id = download.id;
            let reader = manager.get_reader(id);

            let sender = fs_sender.clone();
            if let Some(reader) = reader {
                pool.execute(move || -> Result<(), GenericError> {
                    sender.send(FileStreamMessage::Start(id))?;
                    loop {
                        let chunk = reader.get_chunk();
                        if let None = chunk {
                            break;
                        }
                    }
                    sender.send(FileStreamMessage::Finish(id))?;
                    Ok(())
                });
            }
        }
    });

    loop {
        let mess = fs_receiver.recv()?;

        match mess {
            FileStreamMessage::Start(id) => {
                let stream_state = &mut file_streams[id as usize];
                match stream_state {
                    FileStreamState::NotStarted(_) => {
                        *stream_state = FileStreamState::Working(1);
                    }
                    FileStreamState::Working(x) => {
                        *stream_state = FileStreamState::Working(*x + 1);
                    }
                    FileStreamState::Finished => {
                        return Err(new_custom_error("file stream already finished"));
                    }
                }
            }

            FileStreamMessage::Finish(id) => {
                let stream_state = &mut file_streams[id as usize];
                match stream_state {
                    FileStreamState::NotStarted(_) => {
                        return Err(new_custom_error("file stream not even started"));
                    }
                    FileStreamState::Working(1) => {
                        files_to_send -= 1;
                        *stream_state = FileStreamState::Finished;
                    }
                    FileStreamState::Working(x) if *x > 1 => {
                        *stream_state = FileStreamState::Working(*x - 1);
                    }
                    _ => {
                        return Err(new_custom_error("bad state of file stream"));
                    }
                }
            }
        }

        if files_to_send == 0 {
            break;
        }
    }


    Ok(())
}


