use std::{cell::RefCell, io::Write, path::PathBuf, rc::Rc};

use errors::GenericError;
use net::{JSONReader, TcpEndpoint};

use crate::messages::{DSMessage, DSMessageType, DownloadFile, MessageFiles};

pub fn receive_files(
    mut tcp_endpoint: impl TcpEndpoint) -> Result<(), GenericError> {

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

    let mut stream = tcp_endpoint.get_connection()?;
    let message = DownloadFile{ 
        id: 0
    };
    let json_message = serde_json::to_string(&message)?;

    stream.write(json_message.as_bytes())?;

    Ok(())
}
