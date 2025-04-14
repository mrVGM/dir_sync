use std::{io::{Read, Write}, net::TcpStream, path::PathBuf, sync::{mpsc::{channel, Sender}, Arc, Mutex}};

use common::FileStreamMessage;
use errors::{new_custom_error, GenericError};
use files::{FileChunk, FileWriter, FILE_CHUNK_MAX_SIZE};
use net::{JSONReader, TcpEndpoint};
use thread_pool::ThreadPool;

use crate::{logger::LoggerMessage, messages::{DSMessage, DSMessageType, DownloadFile, MessageFiles}};

enum ReadResult {
    StreamClosed,
    PartiallyRead,
    FullyRead,
}

fn read_bytes(stream: &mut TcpStream, buf: &mut [u8]) -> ReadResult {
    let mut read = 0;

    while read < buf.len() {
        let bytes = stream.read(&mut buf[read..]);
        match bytes {
            Ok(x) if x > 0 => {
                read += x;
            }
            _ => {
                if read == 0 {
                    return ReadResult::StreamClosed;
                }
                else {
                    return ReadResult::PartiallyRead;
                }
            }
        }
    }
    
    ReadResult::FullyRead
}

fn receive_chunk(stream: &mut TcpStream) ->
    Result<Option<FileChunk>, GenericError> {

    let mut buf: Vec<u8> = Vec::with_capacity(FILE_CHUNK_MAX_SIZE);
    buf.resize(FILE_CHUNK_MAX_SIZE, 0);

    {
        let meta_data = &mut buf[..2 * size_of::<u64>()];
        let read = read_bytes(stream, meta_data);
        match read {
            ReadResult::StreamClosed => {
                return Ok(None);
            }
            ReadResult::PartiallyRead => {
                return Err(new_custom_error("stream error"));
            } 
            _ => { }
        }
    }
    let mut data_size = [0; size_of::<u64>()];
    data_size.copy_from_slice(&buf[size_of::<u64>()..2 * size_of::<u64>()]);
    let data_size = u64::from_be_bytes(data_size);

    let res = {
        let data_range = 2 * size_of::<u64>()..2 * size_of::<u64>() + data_size as usize;
        read_bytes(stream, &mut buf[data_range])
    };
    match res {
        ReadResult::FullyRead => {
            let chunk = FileChunk::from_bytes(&buf);
            Ok(Some(chunk))
        }
        _ => {
            Err(new_custom_error("stream error"))
        }
    }
}

pub fn receive_files(
    mut tcp_endpoint: impl TcpEndpoint,
    root: PathBuf,
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

    println!("{} files to receive", files.files.len());

    enum FileStreamState {
        NotStarted,
        Working,
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

    let (slot_send, slot_receive) = channel();
    for _ in 0..4 {
        slot_send.send(())?;
    }

    let (fs_send, fs_receive) = channel();

    let pool = ThreadPool::new(9);
    let pool_clone = pool.clone();
    let logger_clone = logger.clone();

    let root = root.clone();

    let tcp_endpoint = Arc::new(Mutex::new(tcp_endpoint));
    let tcp_endpoint_clone = Arc::clone(&tcp_endpoint);

    pool.execute(move || -> Result<(), GenericError> {
        let tcp_endpoint = &mut *tcp_endpoint_clone.lock().unwrap();

        let pool = pool_clone;
        let logger = logger_clone;

        let writer_pool = ThreadPool::new(4);

        for (i, f) in files.files.iter().enumerate() {
            if f.size == 0 {
                continue;
            }

            let id = i as u32;

            slot_receive.recv()?;
            fs_send.send(FileStreamMessage::Start(id))?;

            let file_path = files::list_to_path(&f.partial_path);
            let file_relative_path = file_path.to_owned();
            let file_path = root.join(file_path);
            let writer = FileWriter::new(
                id,
                f.size,
                &file_path,
                fs_send.clone(),
                writer_pool.clone())?;
            let writer = Arc::new(writer);
            let file_size = f.size;

            for _ in 0..2 {
                let mut stream = tcp_endpoint.get_connection()?;
                let logger = logger.clone();
                let name = file_relative_path.to_str()
                    .ok_or(new_custom_error("no file path"))?
                    .to_owned();

                let writer = Arc::clone(&writer);
                pool.execute(move || -> Result<(), GenericError> {
                    logger.send(LoggerMessage::StartFile {
                        id: id,
                        name: name,
                        size: file_size
                    })?;

                    let download = DownloadFile {
                        id
                    };
                    let download = serde_json::to_string(&download)?;
                    stream.write(download.as_bytes())?;

                    loop {
                        let chunk = receive_chunk(&mut stream)?;
                        match chunk {
                            None => {
                                break;
                            }
                            Some(chunk) => {
                                let size = chunk.size;
                                logger.send(LoggerMessage::AddData {
                                    id: id,
                                    data: size,
                                })?;
                                writer.push_chunk(chunk)?;
                            }
                        }
                    }

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
                        *state = FileStreamState::Working;
                    }
                    _ => {
                        return Err(new_custom_error("download process already started"));
                    } 
                }
            }
            FileStreamMessage::Finish(id) => {
                let state = &mut file_streams[id as usize];
                match state {
                    FileStreamState::NotStarted => {
                        return Err(new_custom_error("download process not started"));
                    }
                    FileStreamState::Working => {
                        *state = FileStreamState::Finished;
                        files_to_receive -= 1;
                        let _ = slot_send.send(());
                        logger.send(LoggerMessage::FinishFile { id: id })?;
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

    drop(tcp_endpoint);

    Ok(())
}
