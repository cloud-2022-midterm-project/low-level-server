use super::PutUpdate;
use crate::{
    handlers::{BindValue, PutMessage},
    maybe::Maybe,
    models::Message,
};
use ahash::AHashMap;

pub struct MutationManager {
    updates_post: AHashMap<String, Message>,
    updates_put: AHashMap<String, PutUpdate>,
    updates_delete: Vec<String>,
}

impl MutationManager {
    pub fn new() -> Self {
        Self {
            updates_post: AHashMap::with_capacity(1000),
            updates_put: AHashMap::with_capacity(1000),
            updates_delete: Vec::with_capacity(1000),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.updates_post.is_empty()
            && self.updates_put.is_empty()
            && self.updates_delete.is_empty()
    }

    pub fn add_post(&mut self, message: Message) {
        self.updates_post.insert(message.uuid.clone(), message);
    }

    pub fn add_delete(&mut self, uuid: &str) {
        // remove from updates_put if it exists
        self.updates_put.remove(uuid);

        // remove from updates_post if it exists
        if matches!(self.updates_post.remove(uuid), None) {
            self.updates_delete.push(uuid.to_string());
        }
    }

    pub fn add_put(&mut self, uuid: &str, params: Vec<BindValue>) {
        // if there's a post update of this uuid, modify it rather than adding to updates_put
        if let Some(m) = self.updates_post.get_mut(uuid) {
            for param in params.iter() {
                match param {
                    BindValue::Author(v) => m.author = v.to_string(),
                    BindValue::Message(v) => m.message = v.to_string(),
                    BindValue::Likes(v) => m.likes = *v,
                    BindValue::HasImage(v) => m.has_image = *v,
                }
            }
        } else {
            // merge with existing update if it exists
            self.updates_put
                .entry(uuid.to_string())
                .and_modify(|m| {
                    for param in params.iter() {
                        match param {
                            BindValue::Author(v) => m.fields.author = Maybe::Value(v.to_string()),
                            BindValue::Message(v) => m.fields.message = Maybe::Value(v.to_string()),
                            BindValue::Likes(v) => m.fields.likes = Maybe::Value(*v),
                            BindValue::HasImage(v) => m.fields.imageUpdate = Maybe::Value(*v),
                        }
                    }
                })
                .or_insert_with(|| {
                    let mut fields = PutMessage::default();
                    for param in params {
                        match param {
                            BindValue::Author(v) => fields.author = Maybe::Value(v),
                            BindValue::Message(v) => fields.message = Maybe::Value(v),
                            BindValue::Likes(v) => fields.likes = Maybe::Value(v),
                            BindValue::HasImage(v) => fields.imageUpdate = Maybe::Value(v),
                        }
                    }
                    PutUpdate {
                        uuid: uuid.to_string(),
                        fields,
                    }
                });
        }
    }

    pub fn updates_post_mut(&mut self) -> &mut AHashMap<String, Message> {
        &mut self.updates_post
    }

    pub fn updates_put_mut(&mut self) -> &mut AHashMap<String, PutUpdate> {
        &mut self.updates_put
    }

    pub fn updates_delete(&self) -> &[String] {
        self.updates_delete.as_ref()
    }

    pub fn clear(&mut self) {
        self.updates_post.clear();
        self.updates_put.clear();
        self.updates_delete.clear();
    }
}

impl Default for MutationManager {
    fn default() -> Self {
        Self::new()
    }
}
