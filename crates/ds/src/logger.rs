use std::{collections::{HashMap, VecDeque}, io::stdout, sync::mpsc::Receiver, time::SystemTime};

use crossterm::{cursor, execute, terminal};
use errors::{new_custom_error, GenericError};

pub enum LoggerMessage {
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
}

#[derive(Debug)]
enum FileState {
    FileProgress {
        last_update: VecDeque<(SystemTime, u64)>,
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

pub fn progress_string(progress: (u64, u64), last_update: &(SystemTime, u64), name: &str) -> String {
    let progress_float = progress.0 as f32 / progress.1 as f32;
    let mut bar: String = "".into();
    let len = 15;
    for i in 0..len {
        let cur = i as f32 / len as f32;
        if cur <= progress_float {
            bar.push('#');
        }
        else {
            bar.push('-');
        }
    }

    let speed = {
        let now = SystemTime::now();
        let (stamp, bytes) = last_update;
        let stamp = stamp.clone();
        let elapsed = now.duration_since(stamp);
        if let Ok(elapsed) = elapsed {
            let elapsed = elapsed.as_millis() as f32 / 1000.0 as f32;
            let diff = progress.0 - bytes;
            diff as f32 / elapsed
        }
        else {
            0.0
        }
    }.floor() as u64;

    let speed = format!(
        "{}/s",
        format_bytes(speed));

    let completion = format!(
        "{}/{}",
        format_bytes(progress.0),
        format_bytes(progress.1));

    format!("[{}] {} {} {}", bar, completion, speed, name)
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
    execute!(stdout, cursor::Hide)?;
    loop {
        let message = receiver.recv()?;
        match message {
            LoggerMessage::StartFile { id, name, size } => {
                let file = find_file(id, &mut files);
                if let None = file {
                    let time_stamp = std::time::SystemTime::now();
                    let mut stamps = VecDeque::new();
                    stamps.push_back((time_stamp, 0));
                    files.insert(id, FileState::FileProgress {
                        last_update: stamps,
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
                    FileState::FileProgress { last_update, name: _, size: _, data } => {
                        *data += add_data;

                        let time_stamp = SystemTime::now();
                        let last_stamp = last_update.back();
                        if let Some(stamp) = last_stamp {
                            let (stamp, _) = stamp;
                            let stamp = stamp.clone();
                            let dur = time_stamp.duration_since(stamp)?;
                            if dur.as_millis() > 1000 {
                                last_update.push_back((time_stamp, *data));
                            }
                        }
                        while last_update.len() > 2 {
                            last_update.pop_front();
                        }
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
                    FileState::FileProgress { last_update: _, name, size: _, data: _ } => {
                        *file_state = FileState::ClosedFile {
                            name: name.to_owned()
                        }
                    }
                    _ => { }
                }
            }
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
                    FileState::FileProgress { last_update: _, name: _, size: _, data: _ } => true,
                    _ => false
                }
            })
            .count();
        for _ in 0..4 - in_progress {
            println!();
        }

        for f in files.values() {
            if let FileState::FileProgress { last_update, name, size, data } = f {
                if let Some(last_update) = last_update.front() {
                    let prog_str = progress_string((*data, *size), last_update, name);
                    println!("{}", prog_str);
                }
            }
        }
        execute!(stdout, crossterm::terminal::EnableLineWrap)?;

        execute!(stdout, terminal::EndSynchronizedUpdate)?;
    }
}
