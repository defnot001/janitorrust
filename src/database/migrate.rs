use std::path::{Path, PathBuf};

use sqlx::PgPool;

pub async fn migrate_db(db_pool: &PgPool) {
    sqlx::query("SET search_path TO public;")
        .execute(db_pool)
        .await
        .expect("Failed to set search path to public");

    let file_paths = get_migration_files();

    for path in file_paths {
        let statements = get_sql_statements(path.clone());

        for statement in statements {
            if let Err(e) = sqlx::query(&statement).execute(db_pool).await {
                tracing::error!("Failed to execute query statement `{statement}`: {e}");
                return;
            }
        }

        tracing::info!(
            "Executed all migration queries from file: {}",
            path.file_name().unwrap().to_str().unwrap()
        );
    }

    tracing::info!("Successfully executed all migrations.");
}

fn get_migration_files() -> Vec<PathBuf> {
    let project_root = std::env::var("CARGO_MANIFEST_DIR")
        .expect("Cannot find CARGO_MANIFEST_DIR enviroment variable");

    let migrations_dir_path = Path::new(&project_root)
        .join("src")
        .join("database")
        .join("migrations");

    let migrations_dir =
        std::fs::read_dir(migrations_dir_path).expect("Cannot read migrations directory");

    let mut sql_files = Vec::new();

    for file in migrations_dir {
        let entry = file.expect("Cannot read file in migrations directory");
        let file_path = entry.path();

        if !file_path.is_file() {
            continue;
        }

        match file_path.extension().and_then(|ext| ext.to_str()) {
            Some("sql") => sql_files.push(file_path),
            _ => continue,
        }
    }

    sql_files.sort();

    sql_files
}

fn get_sql_statements(file_path: PathBuf) -> Vec<String> {
    let file_name = file_path.file_name().unwrap().to_str().unwrap();

    let content = std::fs::read_to_string(&file_path)
        .unwrap_or_else(|_| panic!("Failed to read sql file: `{}`", file_name));

    content
        .split("\n\n")
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
}
