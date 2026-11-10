use crate::{
    api,
    state::{Page, State},
};
use serde_json::json;
use uuid::Uuid;

impl State {
    pub fn show_collection(&mut self) {
        if self.fixture {
            return;
        }
        self.page = Page::Collection;
        self.collection_load(0, false);
    }
    pub fn collection_load(&mut self, offset: usize, debounce: bool) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let query = self.collection.query.text.clone();
        let set = self.collection.set.clone();
        self.collection.error = None;
        self.collection.listing.request(async move {
            if debounce {
                crate::task::delay(300).await;
            }
            let page = api::call(
                &client,
                "inventory.list",
                json!({"limit":100,"offset":offset,"search":query,"set":set}),
            )
            .await?;
            Ok((page, offset))
        });
    }
    pub fn collection_set(&mut self, set: String) {
        self.collection.set = set;
        self.collection.missing = None;
        self.collection.missing_mode = false;
        self.collection.missing_task.cancel();
        self.collection.preview.cancel();
        self.collection.image = None;
        self.collection_load(0, false);
    }
    pub fn collection_missing(&mut self) {
        if self.collection.set.is_empty() {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        let set = self.collection.set.clone();
        self.collection.missing_mode = true;
        self.collection.missing_offset = 0;
        self.collection.image = None;
        self.collection.preview.cancel();
        self.collection.missing_task.request(async move {
            api::call(&client, "inventory.missing", json!({"set":set})).await
        });
    }
    pub fn collection_select(&mut self, selected: usize) {
        self.collection.selected = selected;
        self.collection.copy_offset = 0;
        self.collection.image = None;
        self.collection.image_error = None;
        self.collection.preview.cancel();
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.collection.missing_mode {
            let key = self
                .collection
                .missing
                .as_ref()
                .and_then(|m| m.cards.get(selected))
                .and_then(|c| c.thumbnail_key.clone());
            if let Some(key) = key {
                self.collection
                    .preview
                    .request(async move { api::reference(&client, &key).await });
            }
        } else if let Some(preview) = self
            .collection
            .page
            .as_ref()
            .and_then(|p| p.entries.get(selected))
            .and_then(|e| e.preview.clone())
        {
            let mut params =
                json!({"jobID":preview.batch_id,"sequence":preview.image.sequence,"format":"png"});
            if let Some(edit) = preview.edit_id {
                params["editID"] = json!(edit);
            }
            self.collection
                .preview
                .request(async move { api::thumbnail_params(&client, params).await });
        }
    }
    pub fn collection_open_batch(&mut self, id: Uuid) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.collection
            .opening
            .request(async move { api::call(&client, "jobs.get", json!({"id":id})).await });
    }
    pub(crate) fn poll_collection(&mut self) -> bool {
        let mut changed = false;
        if let Some(result) = self.collection.listing.take() {
            match result {
                Ok((page, offset)) => {
                    self.collection.page = Some(page);
                    self.collection.offset = offset;
                    if !self.collection.missing_mode {
                        self.collection_select(0);
                    }
                }
                Err(error) => self.collection.error = Some(error.to_string()),
            }
            changed = true;
        }
        if let Some(result) = self.collection.missing_task.take() {
            match result {
                Ok(missing) => {
                    self.collection.missing = Some(missing);
                    self.collection_select(0);
                }
                Err(error) => self.collection.error = Some(error.to_string()),
            }
            changed = true;
        }
        if let Some(result) = self.collection.preview.take() {
            match result {
                Ok(bytes) => self.collection.image = Some(self.collection.cache.load(bytes)),
                Err(error) => self.collection.image_error = Some(error.to_string()),
            }
            changed = true;
        }
        if let Some(result) = self.collection.opening.take() {
            match result {
                Ok(record) => {
                    self.page = Page::Library;
                    self.focus_record(record);
                }
                Err(error) => self.collection.error = Some(error.to_string()),
            }
            changed = true;
        }
        changed
    }
}
