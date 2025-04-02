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
    ReportedFile
}

fn format_bytes(bytes: u64) -> String {
    let kb = 1024;
    let mb = kb * 1024;
    let gb = mb * 1024;

    if bytes < mb {
        let f = bytes as f32 / kb as f32;
        let str = format!("{:.2}KB", f);
        return str;
    }

    if bytes < gb {
        let f = bytes as f32 / mb as f32;
        let str = format!("{:.2}MB", f);
        return str;
    }

    let f = bytes as f32 / gb as f32;
    let str = format!("{:.2}GB", f);
    return str;
}

pub fn progress_string(progress: (u64, u64), name: &str) -> String {
    let progress_float = progress.0 as f32 / progress.1 as f32;
    let mut bar: String = "".into();
    let len = 15;
    for i in 0..len {
        let cur = i as f32 / len as f32;
        if cur <= progress_float {
            bar.push('#');
        }
        else {
            bar.push('.');
        }
    }

    let completion = format!(
        "{}/{}",
        format_bytes(progress.0),
        format_bytes(progress.1));

    format!("[{}] {} {}", bar, completion, name)
}

pub fn log_progress(receiver: Receiver<LoggerMessage>)
    -> Result<(), GenericError> {

    let mut files = HashMap::<u32, FileState>::new();

    fn find_file(file_id: u32, files: &mut HashMap<u32, FileState>) -> Option<&mut FileState> {
        let record = files.get_mut(&file_id);
        record
    }

    let mut first_run = true;
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

        execute!(stdout, terminal::BeginSynchronizedUpdate)?;

        if !first_run {
            execute!(stdout, cursor::MoveUp(4))?;
            execute!(stdout, terminal::Clear(terminal::ClearType::FromCursorDown))?;
        }
        first_run = false;

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
                    *f = FileState::ReportedFile;
                }
                _ => {}
            }
        }

        execute!(stdout, crossterm::terminal::DisableLineWrap)?;

        let in_progress = files.values()
            .filter(|f| {
                match f {
                    FileState::FileProgress { name, size, data } => true,
                    _ => false
                }
            })
            .count();
        for _ in 0..4 - in_progress {
            println!();
        }

        for f in files.values() {
            if let FileState::FileProgress { name, size, data } = f {
                let prog_str = progress_string((*data, *size), name);
                println!("{}", prog_str);
            }
        }
        execute!(stdout, crossterm::terminal::EnableLineWrap)?;

        execute!(stdout, terminal::EndSynchronizedUpdate)?;
    }

    Ok(())
}
