use ashes_wiki::{content, create_pool, initialize};
use std::{env, error::Error, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|arg| arg == "-h" || arg == "--help") {
        usage();
        return Ok(());
    }
    let database_url = option_value(&args, "--database-url")
        .or_else(|| env::var("DATABASE_URL").ok())
        .unwrap_or_else(|| "sqlite://ashes.db".to_string());
    let command = args[0].as_str();
    let path = positional_path(&args)?;
    let pool = create_pool(&database_url).await?;
    initialize(&pool).await?;

    match command {
        "apply" => {
            let pack = content::read_pack(&path)?;
            let report =
                content::apply_pack(&pool, &pack, args.iter().any(|a| a == "--force")).await?;
            println!(
                "Applied {}: {} created, {} updated, {} unchanged",
                pack.pack, report.created, report.updated, report.unchanged
            );
        }
        "check" => {
            let pack = content::read_pack(&path)?;
            content::check_pack(&pool, &pack, false).await?;
            println!("Content pack {} is valid and conflict-free.", pack.pack);
        }
        "export" => {
            let name = option_value(&args, "--pack").unwrap_or_else(|| "campaign".into());
            let pack = content::export_pack(&pool, &name).await?;
            content::write_pack(&path, &pack)?;
            println!("Exported {} pages to {}", pack.pages.len(), path.display());
        }
        _ => {
            usage();
            return Err(format!("unknown command: {command}").into());
        }
    }
    Ok(())
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn positional_path(args: &[String]) -> Result<PathBuf, Box<dyn Error>> {
    args.iter()
        .skip(1)
        .find(|arg| !arg.starts_with('-') && !matches_option_value(args, arg))
        .map(PathBuf::from)
        .ok_or_else(|| "a content pack path is required".into())
}

fn matches_option_value(args: &[String], value: &str) -> bool {
    args.windows(2)
        .any(|pair| matches!(pair[0].as_str(), "--database-url" | "--pack") && pair[1] == value)
}

fn usage() {
    eprintln!(
        "Usage:\n  ashes-content apply <pack.json> [--force] [--database-url URL]\n  ashes-content check <pack.json> [--database-url URL]\n  ashes-content export <pack.json> [--pack NAME] [--database-url URL]"
    );
}
