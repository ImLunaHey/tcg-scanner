use scanner_catalogue::{library::LibraryCatalogue, persistent::Documents};
use scanner_types::{cards::CardLanguage, inventory::InventoryCatalogueSource};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: checklist PROVIDER SET_ID ABSOLUTE_CACHE_SQLITE".into());
    }
    let cache = Documents::open(std::path::Path::new(&args[2]))?;
    let library = LibraryCatalogue::cached(cache)?;
    let source = InventoryCatalogueSource {
        provider: args[0].clone(),
        set_id: args[1].clone(),
        language: CardLanguage::English,
    };
    let cards = library.cards(&source).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"count":cards.len(),"sample":cards.iter().take(3).collect::<Vec<_>>()})
        )?
    );
    Ok(())
}
