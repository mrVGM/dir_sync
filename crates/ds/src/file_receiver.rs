use std::{io::{Read, Write}, net::TcpStream, sync::mpsc::{channel, Sender}};

use errors::{new_custom_error, GenericError};
use files::{FileChunk, FILE_CHUNK_MAX_SIZE};
use net::{JSONReader, TcpEndpoint};
use thread_pool::ThreadPool;

use crate::{logger::LoggerMessage, messages::{DSMessage, DSMessageType, DownloadFile, MessageFiles}};

fn receive_chunk(stream: &mut TcpStream) -> Option<FileChunk> {
    let mut buf: Vec<u8> = Vec::with_capacity(FILE_CHUNK_MAX_SIZE);
    buf.resize(FILE_CHUNK_MAX_SIZE, 0);
    let read = stream.read(&mut buf[0..]);

    match read {
        Ok(read) => {
            match read {
                0 => None,
                _ => {
                    let chunk = FileChunk::from_bytes(&buf);
                    Some(chunk)
                }
            }
        }
        Err(_) => None
    }
}

pub fn receive_files(
    mut tcp_endpoint: impl TcpEndpoint,
    logger: Sender<LoggerMessage>) -> Result<(), GenericError> {

    let mut stream = tcp_endpoint.get_connection()?;
    let message = DSMessage {
        message_type: DSMessageType::GetFileList
    };
    let json = serde_json::to_string(&message)?;

    stream.write(json.as_bytes())?;

    let mut reader = JSONReader::new(stream);
    let files = reader.read_json()?;
    let files: MessageFiles = serde_json::from_value(files)?;

    dbg!(&files);

    enum FileStreamState {
        NotStarted,
        Working(u8),
        Finished
    }

    let mut file_streams: Vec<FileStreamState> = files.files
        .iter()
        .map(|f| {
            match f.size {
                0 => FileStreamState::Finished,
                _ => FileStreamState::NotStarted
            }
        }).collect();

    let mut files_to_receive = file_streams.iter()
        .filter(|state| {
            match state {
                FileStreamState::NotStarted => true,
                _ => false
            }
        }).count();

    enum FileStreamMessage {
        Start(u32),
        Finish(u32)
    }

    let (slot_send, slot_receive) = channel();
    for _ in 0..4 {
        slot_send.send(())?;
    }

    let (fs_send, fs_receive) = channel();

    let pool = ThreadPool::new(9);
    let pool_clone = pool.clone();
    let logger_clone = logger.clone();

    pool.execute(move || -> Result<(), GenericError> {
        let pool = pool_clone;
        let logger = logger_clone;

        for (i, f) in files.files.iter().enumerate() {
            if f.size == 0 {
                continue;
            }

            let id = i as u32;

            slot_receive.recv()?;

            for _ in 0..2 {
                let fs_send = fs_send.clone();
                let mut stream = tcp_endpoint.get_connection()?;
                let logger = logger.clone();
                let file_path = files::list_to_path(&f.partial_path);
                let name = file_path.to_str().unwrap().to_owned();
                pool.execute(move || -> Result<(), GenericError> {
                    fs_send.send(FileStreamMessage::Start(id))?;
                    logger.send(LoggerMessage::StartFile {
                        id: id,
                        name: name,
                        size: 0
                    })?;

                    let download = DownloadFile {
                        id
                    };
                    let download = serde_json::to_string(&download)?;
                    stream.write(download.as_bytes())?;

                    loop {
                        let chunk = receive_chunk(&mut stream);
                        match chunk {
                            None => {
                                break;
                            }
                            Some(chunk) => {
                                logger.send(LoggerMessage::AddData {
                                    id: id,
                                    data: chunk.size,
                                })?;
                            }
                        }
                    }
                    fs_send.send(FileStreamMessage::Finish(id))?;

                    Ok(())
                });
            }
        }

        Ok(())
    });

    loop {
        let message = fs_receive.recv()?;

        match message {
            FileStreamMessage::Start(id) => {
                let state = &mut file_streams[id as usize];
                match state {
                    FileStreamState::NotStarted => {
                        *state = FileStreamState::Working(1);
                    }
                    FileStreamState::Working(x) => {
                        *x += 1;
                    }
                    FileStreamState::Finished => {
                        return Err(new_custom_error("download process already finished"));
                    } 
                }
            }
            FileStreamMessage::Finish(id) => {
                let state = &mut file_streams[id as usize];
                match state {
                    FileStreamState::NotStarted => {
                        return Err(new_custom_error("download process not started"));
                    }
                    FileStreamState::Working(1) => {
                        *state = FileStreamState::Finished;
                        files_to_receive -= 1;
                        let _ = slot_send.send(());
                        logger.send(LoggerMessage::FinishFile { id: id })?;
                    }
                    FileStreamState::Working(n) if *n > 1 => {
                        *n -= 1;
                    }
                    _ => {
                        return Err(new_custom_error("download process already finished"));
                    }
                }
            }
        }

        if files_to_receive == 0 {
            break;
        }
    }

    Ok(())
}
