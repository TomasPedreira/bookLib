use std::{fs::OpenOptions, io::Write, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source = PathBuf::from(
        args.next()
            .ok_or("Uso: export_backup <base.sqlite> <backup.json>")?,
    );
    let destination = PathBuf::from(args.next().ok_or("Indica o ficheiro JSON de destino")?);
    if !source.is_file() {
        return Err("A base de dados indicada não existe".into());
    }
    let state = booklib::AppState::open(&source).await?;
    let data = booklib::library::export_data(&state).await?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)?;
    file.write_all(&serde_json::to_vec_pretty(&data)?)?;
    println!("Cópia guardada em {}", destination.display());
    Ok(())
}
