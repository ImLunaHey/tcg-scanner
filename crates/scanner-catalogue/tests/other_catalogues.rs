use async_trait::async_trait;
use scanner_catalogue::{
    JsonTransport,
    library::{ImageTransport, LibraryCatalogue},
};
use scanner_types::{cards::CardLanguage, error::Result, inventory::InventoryCatalogueSource};
use std::sync::{Arc, Mutex};
const ART: &str = "https://cdn.swu-db.com/images/cards/SOR/001.png";
#[derive(Default)]
struct Fixture {
    calls: Mutex<Vec<String>>,
}
#[async_trait]
impl JsonTransport for Fixture {
    async fn get(&self, path: &str, _: usize) -> Result<Vec<u8>> {
        self.calls.lock().unwrap().push(path.into());
        let value = match path {
            "cardinfo.php" => {
                serde_json::json!({"data":[{"id":12345678,"name":"Blue Dragon","type":"Normal Monster","card_sets":[
                {"set_code":"LOB-EN001","set_name":"Legend","set_rarity":"Ultra Rare"},
                {"set_code":"LOB-EN001","set_name":"Legend","set_rarity":"Rare"},
                {"set_code":"LOB-FR001","set_name":"Legend","set_rarity":"Rare"},
                {"set_code":"LOB-AE001","set_name":"Legend","set_rarity":"Rare"}
            ],"card_images":[{"id":12345678}]}]})
            }
            "cardsets.php" => serde_json::json!([{"set_code":"LOB","tcg_date":"2002-03-08"}]),
            "sets" => {
                serde_json::json!([{"setId":"SOR","fullName":"Spark of Rebellion","numberCards":252,"releaseDate":"3/8/24"}])
            }
            "cards/sor" => {
                serde_json::json!({"data":[{"Set":"SOR","Number":"001","Name":"Captain","Subtitle":"A Leader","Type":"Leader","FrontArt":ART}]})
            }
            _ => panic!("unexpected request: {path}"),
        };
        Ok(serde_json::to_vec(&value).unwrap())
    }
}
#[async_trait]
impl ImageTransport for Fixture {
    async fn get(&self, _: &str, _: usize) -> Result<Vec<u8>> {
        panic!("checklists must not download artwork")
    }
}
#[tokio::test]
async fn printing_checklists_keep_ids_deduplicate_rarities_and_respect_language() {
    let fixture = Arc::new(Fixture::default());
    let library = LibraryCatalogue::with_transports(fixture.clone(), fixture.clone());
    let ygo = InventoryCatalogueSource {
        provider: "ygoprodeck".into(),
        set_id: "LOB".into(),
        language: CardLanguage::English,
    };
    let swu = InventoryCatalogueSource {
        provider: "swu-db".into(),
        set_id: "SOR".into(),
        language: CardLanguage::English,
    };
    let (a, b) = tokio::join!(library.cards(&ygo), library.cards(&ygo));
    let cards = a.unwrap();
    assert_eq!(b.unwrap().len(), 2);
    assert_eq!(
        cards.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        [
            "ygoprodeck:12345678:LOB-AE001",
            "ygoprodeck:12345678:LOB-EN001"
        ]
    );
    assert!(cards.iter().all(|c| c.thumbnail_key.is_some()));
    let cards = library.cards(&swu).await.unwrap();
    assert_eq!(cards[0].id, "swu-db:SOR-001");
    assert_eq!(cards[0].name, "Captain — A Leader");
    assert!(cards[0].thumbnail_key.is_some());
    for mut source in [ygo.clone(), swu.clone()] {
        source.language = CardLanguage::Korean;
        assert_eq!(
            library.cards(&source).await.unwrap_err().kind,
            "unsupported"
        );
    }
    let sources = [ygo.clone(), swu.clone()];
    assert!(library.release_dates(&sources).pending);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let dates = library.release_dates(&sources);
            if !dates.pending {
                assert_eq!(dates.dates[&ygo], "2002-03-08");
                assert_eq!(dates.dates[&swu], "2024-03-08");
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut calls = fixture.calls.lock().unwrap().clone();
    calls.sort();
    assert_eq!(calls, ["cardinfo.php", "cards/sor", "cardsets.php", "sets"]);
}
