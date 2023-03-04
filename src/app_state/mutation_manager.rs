use std::path::PathBuf;

use super::PutUpdate;
use crate::{
    handlers::{BindValue, PutMessage},
    maybe::Maybe,
    models::Message,
};
use ahash::AHashSet;
use serde::Serialize;
use tokio::sync::Mutex;

enum Kind {
    Post,
    Put,
}

pub struct MutationManager {
    updates_post: Mutex<AHashSet<String>>,
    updates_put: Mutex<AHashSet<String>>,
    updates_delete: Mutex<Vec<String>>,
    mutation_dir: String,
    get_request_id: Mutex<Option<u8>>,
    post_put_del_vecs: Mutex<(Vec<String>, Vec<String>, Vec<String>)>,
}

#[derive(Serialize)]
pub struct PaginationResult {}

impl MutationManager {
    pub fn new() -> Self {
        Self {
            updates_post: Mutex::new(AHashSet::with_capacity(512)),
            updates_put: Mutex::new(AHashSet::with_capacity(512)),
            updates_delete: Mutex::new(Vec::with_capacity(512)),
            mutation_dir: {
                let path =
                    std::env::var("MUTATIONS_BASE_PATH").expect("MUTATIONS_BASE_PATH must be set");
                // check if this path directory exists
                if !std::path::Path::new(&path).exists() {
                    panic!(
                        "MUTATIONS_BASE_PATH directory does not exist, the given path is {path}."
                    );
                }
                path
            },
            get_request_id: Mutex::new(None),
            post_put_del_vecs: Default::default(),
        }
    }

    pub async fn is_empty(&self) -> bool {
        let (posts, puts, deletes) = (
            self.updates_post.lock().await,
            self.updates_put.lock().await,
            self.updates_delete.lock().await,
        );
        posts.is_empty() && puts.is_empty() && deletes.is_empty()
    }

    fn get_mutation_file_path(&self, uuid: &str, kind: Kind) -> PathBuf {
        let path = std::path::Path::new(&self.mutation_dir).join(uuid);
        match kind {
            Kind::Post => path.join("post.json"),
            Kind::Put => path.join("put.json"),
        }
    }

    pub async fn add_post(&mut self, message: Message) {
        // save the message to the mutation directory
        let path = self.get_mutation_file_path(&message.uuid, Kind::Post);
        std::fs::create_dir_all(&path).expect("Failed to create mutation directory");
        std::fs::write(&path, serde_json::to_string(&message).unwrap())
            .expect("Failed to write mutation file");
        self.updates_post.lock().await.insert(message.uuid);
    }

    pub async fn add_delete(&mut self, uuid: &str) {
        // remove from updates_put if it exists
        self.updates_put.lock().await.remove(uuid);

        // remove from updates_post if it exists
        if self.updates_post.lock().await.remove(uuid) {
            self.updates_delete.lock().await.push(uuid.to_string());
        }
    }

    pub async fn add_put(&mut self, uuid: &str, params: Vec<BindValue>) {
        // if there's a post update of this uuid, modify it rather than adding to updates_put
        if let Some(id) = self.updates_post.lock().await.get(uuid) {
            // get a file handle to the post mutation file
            let path = self.get_mutation_file_path(&id, Kind::Post);
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .expect("Failed to open post mutation file");
            // read from the file into a message
            let mut message: Message =
                serde_json::from_reader(&file).expect("Failed to parse post mutation file");
            // update the message
            for param in params {
                match param {
                    BindValue::Author(v) => message.author = v.to_string(),
                    BindValue::Message(v) => message.message = v.to_string(),
                    BindValue::Likes(v) => message.likes = v,
                    BindValue::HasImage(v) => message.has_image = v,
                }
            }
            // write back to the file handle
            serde_json::to_writer(file, &message).expect("Failed to write post mutation file");
        } else {
            // merge with existing update if it exists
            let path = self.get_mutation_file_path(&uuid, Kind::Put);
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&path)
                .expect("Failed to open mutation file");

            // check if the file is empty
            if file.metadata().unwrap().len() == 0 {
                // if it is, create a new update
                let mut fields = PutMessage::default();
                for param in params {
                    match param {
                        BindValue::Author(v) => fields.author = Maybe::Value(v),
                        BindValue::Message(v) => fields.message = Maybe::Value(v),
                        BindValue::Likes(v) => fields.likes = Maybe::Value(v),
                        BindValue::HasImage(v) => fields.imageUpdate = Maybe::Value(v),
                    }
                }
                let update = PutUpdate {
                    uuid: uuid.to_string(),
                    fields,
                };
                // write the update to the file
                serde_json::to_writer(file, &update).expect("Failed to write put mutation file");
                return;
            }

            // if it's not empty, read the existing update and merge it with the new one

            // read from the file into a message
            let mut update: PutUpdate =
                serde_json::from_reader(&file).expect("Failed to parse put mutation file");
            // merge the updates
            for param in params {
                match param {
                    BindValue::Author(v) => update.fields.author = Maybe::Value(v),
                    BindValue::Message(v) => update.fields.message = Maybe::Value(v),
                    BindValue::Likes(v) => update.fields.likes = Maybe::Value(v),
                    BindValue::HasImage(v) => update.fields.imageUpdate = Maybe::Value(v),
                }
            }
            // write back to the file handle
            serde_json::to_writer(file, &update).expect("Failed to write mutation file");
        }
    }

    pub async fn get_paginated(&mut self, request_id: u8) -> PaginationResult {
        const LIMIT_EACH: u8 = 8;
        let mut self_req_id = self.get_request_id.lock().await;
        if self_req_id.is_none() {
            self_req_id.replace(request_id);
        }
        let (mut posts, mut puts, mut deletes, mut post_put_del_vecs) = (
            self.updates_post.lock().await,
            self.updates_put.lock().await,
            self.updates_delete.lock().await,
            self.post_put_del_vecs.lock().await,
        );
        post_put_del_vecs.0 = posts.drain().collect();
        post_put_del_vecs.1 = puts.drain().collect();
        post_put_del_vecs.2 = std::mem::take(&mut *deletes);

        // gets the pagination pointers for the current request
        // take the first 8 posts, puts, and deletes

        todo!()
    }

    // pub fn updates_post_mut(&mut self) -> &mut AHashMap<String, Message> {
    //     &mut self.updates_post
    // }

    // pub fn updates_put_mut(&mut self) -> &mut AHashMap<String, PutUpdate> {
    //     &mut self.updates_put
    // }

    // pub fn updates_delete(&self) -> &[String] {
    //     self.updates_delete.as_ref()
    // }

    pub async fn clear(&mut self) {
        let (mut posts, mut puts, mut deletes) = (
            self.updates_post.lock().await,
            self.updates_put.lock().await,
            self.updates_delete.lock().await,
        );
        posts.clear();
        puts.clear();
        deletes.clear();
    }
}

impl Default for MutationManager {
    fn default() -> Self {
        Self::new()
    }
}
