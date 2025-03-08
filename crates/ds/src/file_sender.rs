use std::{io::Write, path::PathBuf};

use errors::GenericError;
use files::FileReaderManager;
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

    let stream = tcp_endpoint.wait_for_connection()?;
    let mut json_reader = {
        let stream_clone = stream.try_clone()?;
        JSONReader::new(stream_clone)
    };

    let message = json_reader.read_json()?;
    let req: DownloadFile = serde_json::from_value(message)?;

    let reader = manager.get_reader(req.id);
    if let Some(reader) = reader {
        loop {
            let chunk = reader.get_chunk();
            if let None = chunk {
                break;
            }
        }
    }

    Ok(())
}


