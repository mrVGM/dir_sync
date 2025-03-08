use std::{cell::RefCell, io::Write, path::PathBuf, rc::Rc};

use errors::GenericError;
use files::FileReaderManager;
use net::{JSONReader, TcpEndpoint};

use crate::messages::{DSMessage, DSMessageType, MessageFiles};

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

    match message.message_type {
        DSMessageType::GetFileList => {
            let files = files::get_files_in_dir(&dir)?;
            let files = MessageFiles {
                files
            };

            let json = serde_json::to_string(&files)?;
            main_stream.write(json.as_bytes())?;
        }
    }

    Ok(())
}


