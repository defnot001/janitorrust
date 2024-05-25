use anyhow::Context;
use chrono::{Datelike, Utc};
use poise::serenity_prelude as serenity;
use serenity::{Attachment, CreateAttachment, UserId};
use tokio::fs::{remove_file, write, File};

pub struct FileManager;

impl FileManager {
    pub async fn get(path: &str) -> anyhow::Result<CreateAttachment> {
        let file = File::open(format!("screenshots/{path}")).await?;
        CreateAttachment::file(&file, path).await.context(format!(
            "Failed to create attachment from file at path {path}"
        ))
    }

    pub async fn save(attachment: Attachment, user_id: UserId) -> anyhow::Result<String> {
        let now = Utc::now();
        let date = format!("{}-{}-{}", now.year(), now.month(), now.day());

        let file_ext = match get_file_extension(attachment.filename.to_string()) {
            Some(ext) => ext,
            None => anyhow::bail!(
                "Cannot read file extension from filename {}",
                attachment.filename
            ),
        };

        if file_ext != "jpeg" || file_ext != "jpg" || file_ext != "png" {
            anyhow::bail!("Expected file extensions `jpeg`, `jpg` or `png` but got {file_ext}")
        }

        if attachment.size >= 5_000_000 {
            anyhow::bail!(
                "File size too large. Max file size is 5MB, but got {} bytes",
                attachment.size
            );
        }

        let attachment_content = attachment.download().await?;
        let file_name = format!("{date}_{}.{file_ext}", user_id);

        write(format!("screenshots/{}", &file_name), attachment_content).await?;
        Ok(file_name)
    }

    pub async fn delete(path: &str) -> anyhow::Result<()> {
        remove_file(format!("screenshots/{path}"))
            .await
            .context(format!(
                "Failed to delete screenshot {path} from the file system."
            ))?;

        tracing::info!("Deleted screenshot {path} from the file system.");

        Ok(())
    }
}

fn get_file_extension(file_name: String) -> Option<String> {
    file_name
        .split('.')
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
        .last()
        .map(|last| last.to_string())
}
