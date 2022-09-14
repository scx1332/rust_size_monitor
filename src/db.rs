use rusqlite::Connection;
use anyhow::anyhow;

#[derive(Debug)]
pub struct PathInfo {
    id: i32,
    path: String,
}

fn get_connection() -> anyhow::Result<Connection> {
    Connection::open("size_history.sqlite").map_err(anyhow::Error::from)
}

pub fn add_path_info(path: &str) -> anyhow::Result<()> {
    let conn = get_connection()?;
    let res = conn.execute("INSERT INTO path_info (path) VALUES (?1)", (path,))?;
    Ok(())
}

pub fn get_paths() -> anyhow::Result<Vec<PathInfo>> {
    let conn = get_connection()?;
    let mut stmt = conn.prepare("SELECT id, path FROM path_info")?;
    let iter = stmt.query_map([], |row| {
        Ok(PathInfo {
            id: row.get(0)?,
            path: row.get(1)?,
        })
    })?;
    let mut v : Vec<PathInfo> = Vec::new();
    for el in iter {
        v.push(el?)
    }
    Ok(v)
}

pub fn setup_db() -> anyhow::Result<()> {
    let conn = get_connection()?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS path_info (
        id   INTEGER PRIMARY KEY,
        path TEXT    NOT NULL
    )",
        (), // empty list of parameters.
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS path_entry (
        path_info INTEGER NOT NULL,
        path_size INTEGER NOT NULL,
        FOREIGN KEY(path_info) REFERENCES path_info(id)
    )",
        (), // empty list of parameters.
    )?;

    Ok(())
}
