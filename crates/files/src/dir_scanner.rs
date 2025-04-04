use std::{collections::VecDeque, path::{Path, PathBuf}};

use errors::GenericError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileEntry {
    pub partial_path: Vec<String>,
    pub size: u64
}

pub fn get_files_in_dir(dir: &std::path::Path) -> Result<Vec<FileEntry>, GenericError> {
    let mut to_process = VecDeque::<std::path::PathBuf>::new();
    to_process.push_back(dir.to_path_buf());

    let mut files = vec![];

    while let Some(cur) = to_process.pop_front() {
        let read_dir = std::fs::read_dir(cur)?;

        for item in read_dir {
            let entry = item?;
            let meta = entry.metadata()?;

            if meta.is_file() {
                let path = entry.path();
                let path = path.strip_prefix(dir)?;

                let path_list = path_to_list(&path);
                let file = FileEntry {
                    partial_path: path_list,
                    size: meta.len() as u64
                };

                files.push(file);
                continue;
            }

            if meta.is_dir() {
                let path = entry.path();
                to_process.push_back(path);
            }
        }
    }

    Ok(files)
}

pub fn path_to_list(path: &Path) -> Vec<String> {
    let mut res = vec![];
    let mut cur = Some(path);

    loop {
        match cur {
            None => { 
                break;
            }
            Some(path) => {
                let name = path.file_name();
                if let Some(name) = name {
                    if let Some(name) = name.to_str() {
                        res.insert(0, name.to_string());
                    }
                }
                let parent = path.parent();
                cur = parent;
            }
        }
    }

    res
}

pub fn list_to_path(l: &Vec<String>) -> PathBuf {
    let mut p = PathBuf::new();

    for name in l.iter() {
        let tmp: PathBuf = name.into();
        p = p.join(tmp);
    }

    p
}
