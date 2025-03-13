use std::io::Write;

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

    for (i, f) in files.files.iter().enumerate() {
        if f.size == 0 {
            continue;
        }

        let message = DownloadFile{ 
            id: i as u32
        };
        let json_message = serde_json::to_string(&message)?;

        {
            let mut stream = tcp_endpoint.get_connection()?;
            stream.write(json_message.as_bytes())?;
        }
        {
            let mut stream = tcp_endpoint.get_connection()?;
            stream.write(json_message.as_bytes())?;
        }
    }

    Ok(())
}
