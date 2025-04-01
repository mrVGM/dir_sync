use std::{collections::HashMap, io::stdout, sync::mpsc::Receiver};

use crossterm::{cursor, execute, terminal};
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

#[derive(Debug)]
enum FileState {
    FileProgress {
        name: String,
        size: u64,
        data: u64
    },
    ClosedFile {
        name: String
    },
    ReportedFile {
        name: String
    }
}

pub fn progress_string(progress: f32, name: &str) -> String {
    let mut bar: String = "".into();
    let len = 15;
    for i in 0..len {
        let cur = i as f32 / len as f32;
        if cur <= progress {
            bar.push('#');
        }
        else {
            bar.push('.');
        }
    }

    format!("[{}] {}", bar, name)
}

pub fn log_progress(receiver: Receiver<LoggerMessage>)
    -> Result<(), GenericError> {

    let mut files = HashMap::<u32, FileState>::new();

    // let mut files: Vec<FileState> = vec![];

    fn find_file(file_id: u32, files: &mut HashMap<u32, FileState>) -> Option<&mut FileState> {
        let record = files.get_mut(&file_id);
        record
    }

    let mut temp_prints = vec![];

    let mut stdout = stdout();
    loop {
        let message = receiver.recv()?;
        match message {
            LoggerMessage::CloseLogger => {
                break;
            }
            LoggerMessage::StartFile { id, name, size } => {
                let file = find_file(id, &mut files);
                if let None = file {
                    files.insert(id, FileState::FileProgress {
                        name,
                        size,
                        data: 0
                    });
                }
            }
            LoggerMessage::AddData { id, data: add_data } => {
                let file_state = find_file(id, &mut files)
                    .ok_or(new_custom_error("no file record"))?;

                match file_state {
                    FileState::FileProgress { name, size, data } => {
                        *data += add_data;
                    }
                    _ => {
                        return Err(new_custom_error("file state corrupted"));
                    }
                }
            }
            LoggerMessage::FinishFile { id } => {
                let file_state = find_file(id, &mut files)
                    .ok_or(new_custom_error("no file record"))?;
                match file_state {
                    FileState::FileProgress { name, size: _, data: _ } => {
                        *file_state = FileState::ClosedFile {
                            name: name.to_owned()
                        }
                    }
                    _ => { }
                }
            }
            LoggerMessage::NoMessage => { }
        }

        execute!(stdout, cursor::MoveUp(temp_prints.len() as u16))?;
        execute!(stdout, terminal::Clear(terminal::ClearType::FromCursorDown))?;

        temp_prints.clear();

        let closed = files.values_mut()
            .filter(|x| {
                match x {
                    FileState::ClosedFile { name: _ } => true,
                    _ => false
                } 
            });

        for f in closed {
            match f {
                FileState::ClosedFile { name } => {
                    println!("{} - ready!", &name);
                    *f = FileState::ReportedFile {
                        name: name.to_owned()
                    };
                }
                _ => {}
            }
        }

        execute!(stdout, crossterm::terminal::DisableLineWrap)?;

        for f in files.values() {
            if let FileState::FileProgress { name, size, data } = f {
                let prog = *data as f32 / *size as f32;
                let prog_str = progress_string(prog, name);
                temp_prints.push(prog_str.len());
                println!("{}", prog_str);
            }
        }
        execute!(stdout, crossterm::terminal::EnableLineWrap)?;
    }

    Ok(())
}
