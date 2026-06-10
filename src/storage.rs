use crate::app::DotToDotStudioApp;
use rusqlite::{Connection, Result, params};
use std::path::Path;

// A fully loaded project state read from the SQLite database.
//
// This is still a data-only structure:
// it contains no egui texture, only the raw image bytes and editor data.
pub struct LoadedProjectData {
    pub project_name: String,
    pub origin_url: String,
    pub comment: String,
    pub image_name: Option<String>,
    pub image_size: Option<[usize; 2]>,
    pub image_size_bytes: Option<usize>,
    pub image_bytes: Option<Vec<u8>>,
    pub sequences: Vec<LoadedSequenceData>,
}

// A loaded sequence read from the database.
pub struct LoadedSequenceData {
    pub name: String,
    pub visible: bool,
    pub color: [u8; 4],
    pub line_thickness: f32,
    pub start_value: i32,
    pub points: Vec<LoadedPointData>,
}

// A loaded point read from the database.
pub struct LoadedPointData {
    pub x: f32,
    pub y: f32,
    pub value: i32,
}

// Ensure that the `project` table contains all columns required by the
// current application version.
//
// Why this is needed:
// `CREATE TABLE IF NOT EXISTS` only creates a table when it does not exist yet.
// It does NOT modify an already existing table.
//
// That means older database files may still be missing newer columns such as
// `image_data`. This helper performs a tiny schema migration for that case.
fn ensure_project_table_columns(conn: &Connection) -> Result<()> {
    // Ask SQLite for information about all columns of the `project` table.
    //
    // `PRAGMA table_info(project)` returns one row per column.
    // Column index 1 contains the column name.
    let mut stmt = conn.prepare("PRAGMA table_info(project)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;

    // Track whether the `image_data` column already exists.
    let mut has_image_data = false;

    // Inspect all returned column names.
    for column_name_result in rows {
        let column_name = column_name_result?;

        if column_name == "image_data" {
            has_image_data = true;
        }
    }

    // If the column is missing, add it now.
    //
    // This upgrades older database files to the current schema version
    // without deleting existing project data.
    if !has_image_data {
        conn.execute("ALTER TABLE project ADD COLUMN image_data BLOB", [])?;
    }

    Ok(())
}

fn ensure_sequence_table_columns(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(sequence)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;

    let mut has_line_thickness = false;

    for column_name_result in rows {
        let column_name = column_name_result?;

        if column_name == "line_thickness" {
            has_line_thickness = true;
        }
    }

    if !has_line_thickness {
        conn.execute(
            "ALTER TABLE sequence ADD COLUMN line_thickness REAL NOT NULL DEFAULT 3.0",
            [],
        )?;
    }

    Ok(())
}

// Create all database tables if they do not exist yet.
//
// This schema is intentionally simple for now:
// - one project row
// - zero or one image row
// - many sequence rows
// - many point rows
pub fn initialize_database(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS project (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            name TEXT NOT NULL,
            origin_url TEXT NOT NULL,
            comment TEXT NOT NULL,
            image_name TEXT,
            image_width INTEGER,
            image_height INTEGER,
            image_size_bytes INTEGER,
            image_data BLOB
        );

        CREATE TABLE IF NOT EXISTS sequence (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            sequence_index INTEGER NOT NULL,
            name TEXT NOT NULL,
            visible INTEGER NOT NULL,
            color_r INTEGER NOT NULL,
            color_g INTEGER NOT NULL,
            color_b INTEGER NOT NULL,
            color_a INTEGER NOT NULL,
            line_thickness REAL NOT NULL DEFAULT 3.0,
            start_value INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS point (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            sequence_id INTEGER NOT NULL,
            point_index INTEGER NOT NULL,
            x REAL NOT NULL,
            y REAL NOT NULL,
            value INTEGER NOT NULL,
            FOREIGN KEY(sequence_id) REFERENCES sequence(id) ON DELETE CASCADE
        );
        ",
    )?;

    ensure_project_table_columns(conn)?;
    ensure_sequence_table_columns(conn)?;
    Ok(())
}

// Save the complete current app state into a SQLite database file.
//
// For now we use a very simple strategy:
// - open or create the database
// - create the schema if necessary
// - delete old saved editor data
// - insert the current project/sequences/points
pub fn save_project_to_sqlite<P: AsRef<Path>>(path: P, app: &DotToDotStudioApp) -> Result<()> {
    let mut conn = Connection::open(path)?;
    initialize_database(&conn)?;

    let tx = conn.transaction()?;

    // Remove previous saved content so the file always reflects the current editor state.
    tx.execute("DELETE FROM point", [])?;
    tx.execute("DELETE FROM sequence", [])?;
    tx.execute("DELETE FROM project", [])?;

    // Save the single current project row.
    tx.execute(
        "
        INSERT INTO project (
            id,
            name,
            origin_url,
            comment,
            image_name,
            image_width,
            image_height,
            image_size_bytes,
            image_data
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            1i64,
            &app.project_name,
            &app.origin_url,
            &app.comment,
            &app.image_name,
            app.image_size.map(|size| size[0] as i64),
            app.image_size.map(|size| size[1] as i64),
            app.image_size_bytes.map(|size| size as i64),
            &app.image_bytes,
        ],
    )?;

    // Save all sequences and their points.
    for (sequence_index, sequence) in app.sequences.iter().enumerate() {
        tx.execute(
            "
            INSERT INTO sequence (
                sequence_index,
                name,
                visible,
                color_r,
                color_g,
                color_b,
                color_a,
                line_thickness,
                start_value
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                sequence_index as i64,
                sequence.name,
                if sequence.visible { 1i64 } else { 0i64 },
                sequence.color.r() as i64,
                sequence.color.g() as i64,
                sequence.color.b() as i64,
                sequence.color.a() as i64,
                sequence.line_thickness as f64,
                sequence.start_value as i64,
            ],
        )?;

        let sequence_id = tx.last_insert_rowid();

        for (point_index, point) in sequence.points.iter().enumerate() {
            tx.execute(
                "
                INSERT INTO point (
                    sequence_id,
                    point_index,
                    x,
                    y,
                    value
                )
                VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    sequence_id,
                    point_index as i64,
                    point.position.x as f64,
                    point.position.y as f64,
                    point.value as i64,
                ],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

// Load the complete project state from a SQLite database file.
//
// This reads:
// - project metadata
// - embedded image bytes
// - all sequences
// - all points
pub fn load_project_from_sqlite<P: AsRef<Path>>(path: P) -> Result<LoadedProjectData> {
    let conn = Connection::open(path)?;
    initialize_database(&conn)?;

    // Load the single project row.
    let mut stmt = conn.prepare(
        "
        SELECT
            name,
            origin_url,
            comment,
            image_name,
            image_width,
            image_height,
            image_size_bytes,
            image_data
        FROM project
        WHERE id = 1
        ",
    )?;

    let project_row = stmt.query_row([], |row| {
        let image_width: Option<i64> = row.get(4)?;
        let image_height: Option<i64> = row.get(5)?;

        let image_size = match (image_width, image_height) {
            (Some(width), Some(height)) => Some([width as usize, height as usize]),
            _ => None,
        };

        Ok(LoadedProjectData {
            project_name: row.get(0)?,
            origin_url: row.get(1)?,
            comment: row.get(2)?,
            image_name: row.get(3)?,
            image_size,
            image_size_bytes: row.get::<_, Option<i64>>(6)?.map(|v| v as usize),
            image_bytes: row.get(7)?,
            sequences: Vec::new(),
        })
    })?;

    let mut loaded_project = project_row;

    // Load all sequences in their saved order.
    let mut sequence_stmt = conn.prepare(
        "
        SELECT
            id,
            sequence_index,
            name,
            visible,
            color_r,
            color_g,
            color_b,
            color_a,
            line_thickness,
            start_value
        FROM sequence
        ORDER BY sequence_index ASC
        ",
    )?;

    let sequence_rows = sequence_stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?, // database sequence id
            LoadedSequenceData {
                name: row.get(2)?,
                visible: row.get::<_, i64>(3)? != 0,
                color: [
                    row.get::<_, i64>(4)? as u8,
                    row.get::<_, i64>(5)? as u8,
                    row.get::<_, i64>(6)? as u8,
                    row.get::<_, i64>(7)? as u8,
                ],
                line_thickness: row.get::<_, f64>(8)? as f32,
                start_value: row.get::<_, i64>(9)? as i32,
                points: Vec::new(),
            },
        ))
    })?;

    for sequence_row in sequence_rows {
        let (sequence_id, mut sequence) = sequence_row?;

        // Load all points for the current sequence in their saved order.
        let mut point_stmt = conn.prepare(
            "
            SELECT
                x,
                y,
                value
            FROM point
            WHERE sequence_id = ?1
            ORDER BY point_index ASC
            ",
        )?;

        let point_rows = point_stmt.query_map([sequence_id], |row| {
            Ok(LoadedPointData {
                x: row.get::<_, f64>(0)? as f32,
                y: row.get::<_, f64>(1)? as f32,
                value: row.get::<_, i64>(2)? as i32,
            })
        })?;

        for point_row in point_rows {
            sequence.points.push(point_row?);
        }

        loaded_project.sequences.push(sequence);
    }

    Ok(loaded_project)
}
