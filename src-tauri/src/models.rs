use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Connection {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TableInfo {
    pub name: String,
    pub size: u64,
    pub is_view: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DumpTableRequest {
    pub name: String,
    pub structure: bool,
    pub content: bool,
    pub drop: bool,
}

#[derive(Serialize, Clone)]
pub struct DumpProgress {
    pub table: String,
    pub status: String,
    pub completed: usize,
    pub total: usize,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct DumpResult {
    pub output_dir: String,
    pub final_file: String,
    pub table_count: usize,
    pub individual_size_sum: u64,
    pub final_file_size: u64,
    pub verified: bool,
}
