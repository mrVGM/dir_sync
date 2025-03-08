use files::FileEntry;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum DSMessageType {
    GetFileList
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DSMessage {
    pub message_type: DSMessageType
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MessageFiles {
    pub files: Vec<FileEntry>
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DownloadFile {
    pub id: u32
}

