use std::{fmt, path::PathBuf};

use super::PutUpdate;
use crate::{
    handlers::{BindValue, CompleteMessage, CompletePutUpdate, PaginationMetadata, PutMessage},
    image_store::ImageStore,
    maybe::Maybe,
    models::Message,
};
use ahash::AHashSet;
use serde::Serialize;
use tokio::sync::MutexGuard;

#[derive(Serialize, Debug)]
enum Kind {
    #[serde(rename = "post")]
    Post,
    #[serde(rename = "put")]
    Put,
    #[serde(rename = "delete")]
    Delete,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Post => write!(f, "post"),
            Kind::Put => write!(f, "put"),
            Kind::Delete => write!(f, "delete"),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct Entry {
    kind: Kind,
    uuid: String,
}

#[derive(Serialize, Default, Debug)]
pub struct MutationResults {
    pub posts: Vec<CompleteMessage>,
    pub puts: Vec<CompletePutUpdate>,
    pub deletes: Vec<String>,
}

pub struct MutationManager {
    updates_post: AHashSet<String>,
    updates_put: AHashSet<String>,
    updates_delete: Vec<String>,
    mutation_dir: PathBuf,
    updates_all: Vec<Entry>,
    page_size: usize,
    image_store: ImageStore,
}

impl MutationManager {
    pub fn new(page_size: usize, image_store: ImageStore) -> Self {
        Self {
            updates_post: AHashSet::with_capacity(512),
            updates_put: AHashSet::with_capacity(512),
            updates_delete: Vec::with_capacity(512),
            mutation_dir: {
                let path =
                    std::env::var("MUTATIONS_BASE_PATH").expect("MUTATIONS_BASE_PATH must be set");
                let path = std::path::Path::new(&path).to_path_buf();
                // check if this path directory exists
                if !std::path::Path::new(&path).exists() {
                    panic!(
                        "MUTATIONS_BASE_PATH directory does not exist, the given path is {path:?}."
                    );
                }
                path
            },
            updates_all: Default::default(),
            page_size,
            image_store,
        }
    }

    pub fn is_pagination_empty(&self) -> bool {
        self.updates_all.is_empty()
    }

    pub fn is_empty_for_pagination(&self) -> bool {
        self.updates_post.is_empty()
            && self.updates_put.is_empty()
            && self.updates_delete.is_empty()
    }

    pub fn add_post(&mut self, message: Message) {
        // save the message to the mutation directory
        self.create_dir_all(&message.uuid);
        let path = self.get_mutation_file_path(&message.uuid, Kind::Post);
        std::fs::write(path, serde_json::to_string(&message).unwrap()).unwrap();
        self.updates_post.insert(message.uuid);
    }

    pub fn add_delete(&mut self, uuid: &str) {
        // remove from updates_put if it exists
        self.updates_put.remove(uuid);

        // remove from updates_post if it exists
        if self.updates_post.remove(uuid) {
            self.updates_delete.push(uuid.to_string());
        }
    }

    pub fn add_put(&mut self, uuid: &str, params: Vec<BindValue>) {
        self.create_dir_all(uuid);

        // if there's a post update of this uuid, modify it rather than adding to updates_put
        if let Some(id) = self.updates_post.get(uuid) {
            // get a file handle to the post mutation file
            let path = self.get_mutation_file_path(id, Kind::Post);
            let file_content = std::fs::read_to_string(&path).unwrap();
            // read from the file into a message
            let mut message: Message =
                serde_json::from_str(&file_content).expect("Failed to parse post mutation file");
            // update the message
            for param in params {
                match param {
                    BindValue::Author(v) => message.author = v.to_string(),
                    BindValue::Message(v) => message.message = v.to_string(),
                    BindValue::Likes(v) => message.likes = v,
                    BindValue::HasImage(v) => message.has_image = v,
                }
            }
            // write back to the file
            std::fs::write(&path, serde_json::to_string(&message).unwrap()).unwrap();
        } else {
            // merge with existing update if it exists
            let path = self.get_mutation_file_path(uuid, Kind::Put);

            // check if the file exists
            if self.updates_put.get(uuid).is_some() {
                // if it's not empty, read the existing update and merge it with the new one

                // read from the file into a message
                let mut update: PutUpdate =
                    serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
                        .expect("Failed to parse put mutation file");

                // merge the updates
                for param in params {
                    match param {
                        BindValue::Author(v) => update.fields.author = Maybe::Value(v),
                        BindValue::Message(v) => update.fields.message = Maybe::Value(v),
                        BindValue::Likes(v) => update.fields.likes = Maybe::Value(v),
                        BindValue::HasImage(v) => update.fields.imageUpdate = Maybe::Value(v),
                    }
                }

                // write back to the file
                std::fs::write(&path, serde_json::to_string(&update).unwrap())
                    .expect("Failed to write put mutation file");
                return;
            }

            // if it is empty, create a new update
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

            // file content
            let content = serde_json::to_string(&update).unwrap();

            // write the update to the file
            std::fs::write(&path, content).expect("Failed to write mutation file");

            // add to updates_put
            self.updates_put.insert(uuid.to_string());
        }
    }

    pub fn get_pagination_meta(&mut self) -> PaginationMetadata {
        let posts: Vec<_> = self
            .updates_post
            .drain()
            .map(|uuid| Entry {
                kind: Kind::Post,
                uuid,
            })
            .collect();
        self.updates_all.extend(posts);
        let puts: Vec<_> = self
            .updates_put
            .drain()
            .map(|uuid| Entry {
                kind: Kind::Put,
                uuid,
            })
            .collect();
        self.updates_all.extend(puts);
        let del: Vec<_> = self
            .updates_delete
            .iter()
            .map(|uuid| Entry {
                kind: Kind::Delete,
                uuid: uuid.to_string(),
            })
            .collect();
        self.updates_delete.clear();
        self.updates_all.extend(del);

        PaginationMetadata::new(self.updates_all.len(), self.page_size)
    }

    pub fn get(&mut self, mut triggered_pagination: MutexGuard<bool>) -> MutationResults {
        let mut result = MutationResults::default();

        // extract `page_size` updates from `updates_all` add them to `result`
        for _ in 0..self.page_size {
            if let Some(entry) = self.updates_all.pop() {
                match entry.kind {
                    Kind::Post => {
                        let path = self.get_mutation_file_path(&entry.uuid, Kind::Post);
                        let update = std::fs::read_to_string(&path)
                            .expect("Failed to read post mutation file");
                        let message: Message = serde_json::from_str(&update)
                            .expect("Failed to parse post mutation file");
                        let image = match message.has_image {
                            true => self.image_store.get(&entry.uuid),
                            false => None,
                        };
                        let complete_message = CompleteMessage::new(message, image);
                        result.posts.push(complete_message);

                        // remove the post mutation file
                        std::fs::remove_file(&path).expect("Failed to remove post mutation file");
                        // remove dir as well
                        std::fs::remove_dir_all(self.get_mutation_file_dir(&entry.uuid))
                            .expect("Failed to remove post dir");
                    }
                    Kind::Put => {
                        let path = self.get_mutation_file_path(&entry.uuid, Kind::Put);
                        let update = std::fs::read_to_string(&path)
                            .expect("Failed to read put mutation file");
                        let update: PutUpdate = serde_json::from_str(&update)
                            .expect("Failed to parse put mutation file");
                        let image = match update.fields.imageUpdate {
                            Maybe::Value(true) => self.image_store.get(&entry.uuid),
                            _ => None,
                        };
                        let complete_update = CompletePutUpdate::new(update, image);
                        result.puts.push(complete_update);

                        // remove the put mutation file
                        std::fs::remove_file(&path).expect("Failed to remove put mutation file");
                        // remove dir as well
                        std::fs::remove_dir_all(self.get_mutation_file_dir(&entry.uuid))
                            .expect("Failed to remove put dir");
                    }
                    Kind::Delete => {
                        result.deletes.push(entry.uuid);
                    }
                }
            } else {
                // pagination is done
                *triggered_pagination = false;
                break;
            }
        }

        result
    }

    // pub fn clear(&mut self) {
    //     self.updates_post.clear();
    //     self.updates_put.clear();
    //     self.updates_delete.clear();
    // }

    fn get_mutation_file_dir(&self, uuid: &str) -> PathBuf {
        std::path::Path::new(&self.mutation_dir).join(uuid)
    }

    /// Creates a directory for the mutation files
    fn create_dir_all(&self, uuid: &str) {
        std::fs::create_dir_all(self.get_mutation_file_dir(uuid))
            .expect("Failed to create mutation directory");
    }

    fn get_mutation_file_path(&self, uuid: &str, kind: Kind) -> PathBuf {
        self.get_mutation_file_dir(uuid)
            .join(format!("{}.json", kind))
    }
}
