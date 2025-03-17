use std::sync::mpsc::Receiver;

use errors::{new_custom_error, GenericError};

pub enum LoggerMessage {
    NoMessage,
    StartFile {
        id: u32,
        name: String,
        size: u64
    },
    AddData {
        id: u32,
        data: u64
    },
    FinishFile {
        id: u32
    },
    CloseLogger,
}

enum FileState {
    FileProgress {
        id: u32,
        name: String,
        size: u64,
        data: u64
    },
    ClosedFile {
        name: String
    },
}

fn log_progress(receiver: Receiver<LoggerMessage>)
    -> Result<(), GenericError> {

    let mut files: Vec<FileState> = vec![];

    fn find_file(file_id: u32, files: &mut Vec<FileState>) -> Result<&mut FileState, GenericError> {
        for f in files.iter_mut() {
            match f {
                FileState::FileProgress { id, name: _, size: _, data: _ } => {
                    if file_id == *id {
                        return Ok(f);
                    }
                }
                _ => {
                    continue;
                }
            }
        }

        Err(new_custom_error("file state not found"))
    }

    loop {
        let message = receiver.recv()?;
        match message {
            LoggerMessage::CloseLogger => {
                break;
            }
            LoggerMessage::StartFile { id, name, size } => {
                files.push(FileState::FileProgress {
                    id,
                    name,
                    size,
                    data: 0
                });
            }
            LoggerMessage::AddData { id, data: add_data } => {
                let file_state = find_file(id, &mut files)?;

                match file_state {
                    FileState::FileProgress { id: _, name: _, size: _, data } => {
                        *data += add_data;
                    }
                    _ => {
                        return Err(new_custom_error("file state corrupted"));
                    }
                }
            }
            LoggerMessage::FinishFile { id } => {
                let file_state = find_file(id, &mut files)?;
                match file_state {
                    FileState::FileProgress { id: _, name, size: _, data: _ } => {
                        *file_state = FileState::ClosedFile {
                            name: name.to_owned()
                        }
                    }
                    _ => {
                        return Err(new_custom_error("file state corrupted"));
                    }
                }
            }
            LoggerMessage::NoMessage => { }
        }

        for f in files.iter() {
            match f {
                FileState::ClosedFile { name } => {
                    println!("File {}", name);
                }
                _ => {}
            }
        }

        files = files.into_iter()
            .filter(|f| {
                match f {
                    FileState::FileProgress { id: _, name: _, size: _, data: _ } => true,
                    _ => false
                }
            }).collect();
    }

    Ok(())
}
