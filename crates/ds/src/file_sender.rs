use std::{io::Write, net::TcpStream, path::PathBuf, sync::mpsc::{channel, Sender}};

use errors::{new_custom_error, GenericError};
use files::{FileReaderManager, ReaderResult};
use net::{JSONReader, TcpEndpoint};
use thread_pool::ThreadPool;

use crate::{logger::LoggerMessage, messages::{DSMessage, DSMessageType, DownloadFile, MessageFiles}};

fn write_bytes(stream: &mut TcpStream, buf: &[u8]) ->
    Result<(), GenericError> {
    let mut written = 0;

    while written < buf.len() {
        let n = stream.write(&buf[written..])?;
        if n == 0 {
            return Err(new_custom_error("stream write error"));
        }
        written += n;
    }

    Ok(())
}

pub fn send_files(
    mut tcp_endpoint: impl TcpEndpoint,
    dir: PathBuf,
    logger: Sender<LoggerMessage>) -> Result<(), GenericError> {

    let mut main_stream = tcp_endpoint.wait_for_connection()?;
    let mut reader = {
        let main_stream_clone = main_stream.try_clone()?;
        JSONReader::new(main_stream_clone)
    };

    let json = reader.read_json()?;
    let message: DSMessage = serde_json::from_value(json)?;
    let files = files::get_files_in_dir(&dir)?;

    #[derive(Debug)]
    enum FileStreamState {
        NotStarted,
        Working(u8),
        Finished
    }

    let mut file_streams: Vec<FileStreamState> = files.iter()
        .map(|f| {
            match f.size {
                0 => FileStreamState::Finished,
                _ => FileStreamState::NotStarted
            }
        }).collect();

    let mut files_to_send = file_streams.iter()
        .filter(|f| {
            match f {
                FileStreamState::NotStarted => true,
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


    #[derive(Debug)]
    enum FileStreamMessage {
        Start(u32),
        Finish(u32)
    }
    let (fs_sender, fs_receiver) = channel();
    let fs_sender_clone = fs_sender.clone();

    let pool = ThreadPool::new(9);
    let pool_clone = pool.clone();

    let logger_clone = logger.clone();

    let (slot_sender, slot_receiver) = channel();
    for _ in 0..4 {
        slot_sender.send(())?;
    }

    pool.execute(move || -> Result<(), GenericError> {
        let fs_sender = fs_sender_clone;
        let pool = pool_clone;
        let logger = logger_clone;

        loop {
            let mut stream = tcp_endpoint.wait_for_connection()?;

            let mut reader = { 
                let stream_clone = stream.try_clone()?;
                JSONReader::new(stream_clone)
            };
            let message = reader.read_json()?;

            let download: DownloadFile = serde_json::from_value(message)?;
            let id = download.id;
            let reader = { 
                let reader_result = manager.get_reader(id);
                match reader_result {
                    ReaderResult::NoReader => None,
                    ReaderResult::FirstInstance(reader) => {
                        slot_receiver.recv()?;
                        Some(reader)
                    }
                    ReaderResult::Instance(reader) => {
                        Some(reader)
                    }
                }
            };

            let sender = fs_sender.clone();
            let logger = logger.clone();
            if let Some(reader) = reader {
                pool.execute(move || -> Result<(), GenericError> {
                    sender.send(FileStreamMessage::Start(id))?;
                    logger.send(LoggerMessage::StartFile {
                        id: id,
                        name: reader.name.to_owned(), 
                        size: reader.size
                    })?;
                    loop {
                        let chunk = reader.get_chunk();

                        match chunk {
                            Some(chunk) => {
                                if chunk.size == 0 {
                                    return Err(new_custom_error("zero size chunk"));
                                }
                                let buf = chunk.to_bytes();
                                write_bytes(&mut stream, &buf)?;
                                logger.send(LoggerMessage::AddData { 
                                    id: id,
                                    data: chunk.size
                                })?;
                            }
                            None => {
                                break;
                            }
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
                    FileStreamState::NotStarted => {
                        *stream_state = FileStreamState::Working(1);
                    }
                    FileStreamState::Working(x) => {
                        *x += 1;
                    }
                    FileStreamState::Finished => { }
                }
            }

            FileStreamMessage::Finish(id) => {
                let stream_state = &mut file_streams[id as usize];
                match stream_state {
                    FileStreamState::NotStarted => {
                        return Err(new_custom_error("file stream not even started"));
                    }
                    FileStreamState::Working(1) => {
                        files_to_send -= 1;
                        *stream_state = FileStreamState::Finished;
                        logger.send(LoggerMessage::FinishFile {
                            id
                        })?;

                        slot_sender.send(())?;
                    }
                    FileStreamState::Working(x) if *x > 1 => {
                        *x -= 1;
                    }
                    _ => { }
                }
            }
        }

        if files_to_send == 0 {
            break;
        }
    }


    Ok(())
}


