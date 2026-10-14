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
}

#[derive(Serialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub size: u64,
}

#[derive(Clone, Deserialize)]
pub struct DumpTableSelection {
    pub name: String,
    pub structure: bool,
    pub content: bool,
    pub drop: bool,
}
