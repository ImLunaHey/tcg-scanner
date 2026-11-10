use scanner_catalogue::TcgDex;
use scanner_types::cards::CardLanguage;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = std::env::args().nth(1).ok_or("usage: card CARD_ID")?;
    let detail = TcgDex::new(CardLanguage::English)?.detail(&id).await?;
    println!("{}", serde_json::to_string_pretty(detail.as_ref())?);
    Ok(())
}
