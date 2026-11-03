use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub limit: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableColumn {
    pub name: String,
    pub column_type: String,
    pub nullable: bool,
    pub key: String,
    pub default: Option<String>,
    pub extra: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub columns: Vec<TableColumn>,
    pub create_sql: String,
}
