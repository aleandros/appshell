use std::io::{self, Read};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().take(4096).read_to_string(&mut input)?;
    let account: appshell_api::models::CreateAdmin = serde_json::from_str(&input)?;
    let url = std::env::var("DATABASE_URL")?;
    let pool = appshell_api::db::connect(&url)
        .map_err(|e| format!("Database connection failed: {e:?}"))?;
    appshell_api::db::migrate(pool.clone())
        .await
        .map_err(|e| format!("Migration failed: {e:?}"))?;
    let admin = appshell_api::bootstrap_admin(pool, account)
        .await
        .map_err(|e| format!("Bootstrap failed: {e:?}"))?;
    println!("Created administrator {} ({})", admin.email, admin.id);
    Ok(())
}
