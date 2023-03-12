use crate::{
    app_state::PutUpdate,
    handlers::{BindValue, CompleteMessage, PaginationMetadata, PaginationType, PutMessage},
    image,
    maybe::Maybe,
    models::Message,
};
use ahash::AHashSet;
use serde::Serialize;
use std::{collections::VecDeque, fmt, path::PathBuf};

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

#[derive(Serialize, Debug)]
pub struct PutDeleteUpdate {
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    put: Maybe<CompletePutUpdate>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    delete: Maybe<String>,
}

#[derive(Serialize, Debug)]
pub struct MutationResults {
    pub posts: Vec<CompleteMessage>,
    pub puts_deletes: Vec<PutDeleteUpdate>,
    pub done: bool,
}

impl MutationResults {
    pub fn new() -> Self {
        Self {
            done: false,
            posts: Vec::with_capacity(32),
            puts_deletes: Vec::with_capacity(32),
        }
    }
}

impl Default for MutationResults {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Debug)]
pub struct CompletePutUpdate {
    uuid: String,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    author: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    message: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    likes: Maybe<i32>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    image: Maybe<String>,
}

impl CompletePutUpdate {
    pub fn new(update: PutUpdate) -> Self {
        Self {
            uuid: update.uuid,
            author: update.fields.author,
            message: update.fields.message,
            likes: update.fields.likes,
            image: match update.fields.imageUpdate {
                // Deciding if an image is updated with new content or it is removed
                Maybe::Value(true) => {
                    if let Maybe::Value(image) = update.fields.image {
                        Maybe::Value(image)
                    } else {
                        Maybe::Value(String::new()) // tell the client to remove the image by sending an empty string
                    }
                }
                _ => Maybe::Absent,
            },
        }
    }
}

pub struct MutationManager {
    updates_post: AHashSet<String>,
    updates_put: AHashSet<String>,
    updates_delete: Vec<String>,
    mutation_dir: PathBuf,
    updates_all: VecDeque<Entry>,
    page_size: usize,
}

impl MutationManager {
    pub fn new(page_size: usize) -> Self {
        let s = Self {
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
            updates_all: VecDeque::with_capacity(50_000usize.next_power_of_two()),
            page_size,
        };
        MutationManager::clear_dir(&s.mutation_dir).ok();
        s
    }

    pub fn is_pagination_empty(&self) -> bool {
        self.updates_all.is_empty()
    }

    pub fn is_empty_for_pagination(&self) -> bool {
        self.updates_post.is_empty()
            && self.updates_put.is_empty()
            && self.updates_delete.is_empty()
    }

    pub fn add_post(&mut self, message: CompleteMessage) {
        // save the message to the mutation directory
        let path = self.get_mutation_file_path(&message.uuid);
        let encoded = bincode::serialize(&message).unwrap();
        std::fs::write(path, encoded).unwrap();
        self.updates_post.insert(message.uuid);
    }

    pub fn add_delete(&mut self, uuid: &str) {
        // remove from updates_put if it exists
        self.updates_put.remove(uuid);

        // remove from updates_post if it exists
        if !self.updates_post.remove(uuid) {
            self.updates_delete.push(uuid.to_string());
        }
    }

    pub fn add_put(&mut self, uuid: &str, params: Vec<BindValue>) {
        let path = self.get_mutation_file_path(uuid);

        // if there's a post update of this uuid, modify it rather than adding to updates_put
        if self.updates_post.get(uuid).is_some() {
            // retrieve the message from the file
            let file_content = std::fs::read(&path).expect("Failed to read put mutation file");
            let mut message: Message =
                bincode::deserialize(&file_content).expect("Failed to deserialize message");

            // update the message
            for param in params {
                match param {
                    BindValue::Author(v) => message.author = v.to_string(),
                    BindValue::Message(v) => message.message = Some(v.to_string()),
                    BindValue::Likes(v) => message.likes = v,
                    BindValue::HasImage(v) => message.has_image = v,
                    BindValue::ImageUpdate(_) => (),
                    BindValue::Image(_) => (),
                }
            }

            // write back to the file
            let encoded = bincode::serialize(&message).unwrap();
            std::fs::write(&path, encoded).unwrap();
        } else {
            // merge with existing update if it exists
            // check if the file exists
            if self.updates_put.get(uuid).is_some() {
                // if it's not empty, read the existing update and merge it with the new one

                // read from the file into a message
                let file_content = std::fs::read(&path).expect("Failed to read put mutation file");

                // do not use `bincode` here because it fails with the `Maybe` type
                let mut update: PutUpdate = serde_json::from_slice(&file_content)
                    .expect("Failed to deserialize put mutation file");

                // merge the updates
                for param in params {
                    match param {
                        BindValue::Author(v) => update.fields.author = Maybe::Value(v),
                        BindValue::Message(v) => update.fields.message = Maybe::Value(v),
                        BindValue::Likes(v) => update.fields.likes = Maybe::Value(v),
                        BindValue::ImageUpdate(v) => update.fields.imageUpdate = Maybe::Value(v),
                        BindValue::HasImage(_) => (),
                        BindValue::Image(v) => update.fields.image = Maybe::Value(v),
                    }
                }

                // write back to the file
                let encoded = bincode::serialize(&update).unwrap();
                std::fs::write(&path, encoded).unwrap();
                return;
            }

            // if it is empty, create a new update
            let mut fields = PutMessage::default();
            for param in params {
                match param {
                    BindValue::Author(v) => fields.author = Maybe::Value(v),
                    BindValue::Message(v) => fields.message = Maybe::Value(v),
                    BindValue::Likes(v) => fields.likes = Maybe::Value(v),
                    BindValue::ImageUpdate(v) => fields.imageUpdate = Maybe::Value(v),
                    BindValue::HasImage(_) => (),
                    BindValue::Image(v) => fields.image = Maybe::Value(v),
                }
            }
            let update = PutUpdate {
                uuid: uuid.to_string(),
                fields,
            };

            // file content
            // do not use `bincode` here because it fails with the `Maybe` type
            let encoded = serde_json::to_vec(&update).unwrap();

            // write the update to the file
            std::fs::write(&path, encoded).expect("Failed to write mutation file");

            // add to updates_put
            self.updates_put.insert(uuid.to_string());
        }
    }

    pub fn get_pagination_meta(&mut self) -> PaginationMetadata {
        let mut posts: Vec<_> = self
            .updates_post
            .drain()
            .map(|uuid| Entry {
                kind: Kind::Post,
                uuid,
            })
            .collect();
        // sort posts by uuid
        posts.sort_by(|a, b| a.uuid.cmp(&b.uuid));
        self.updates_all.extend(posts);

        let mut puts_deletes =
            Vec::with_capacity(self.updates_put.len() + self.updates_delete.len());
        let puts: Vec<_> = self
            .updates_put
            .drain()
            .map(|uuid| Entry {
                kind: Kind::Put,
                uuid,
            })
            .collect();
        puts_deletes.extend(puts);
        let del: Vec<_> = self
            .updates_delete
            .iter()
            .map(|uuid| Entry {
                kind: Kind::Delete,
                uuid: uuid.to_string(),
            })
            .collect();
        self.updates_delete.clear();
        puts_deletes.extend(del);
        // sort puts_deletes by uuid
        puts_deletes.sort_by(|a, b| a.uuid.cmp(&b.uuid));
        self.updates_all.extend(puts_deletes);

        PaginationMetadata::new(
            self.updates_all.len(),
            self.page_size,
            PaginationType::Cache,
        )
    }

    pub fn get(&mut self, image_base_path: &PathBuf) -> MutationResults {
        let mut result = MutationResults::default();

        // extract `page_size` updates from `updates_all` add them to `result`
        for _ in 0..self.page_size {
            if let Some(entry) = self.updates_all.pop_front() {
                let path = self.get_mutation_file_path(&entry.uuid);
                match entry.kind {
                    Kind::Post => {
                        let update =
                            std::fs::read(&path).expect("Failed to read post mutation file");
                        let message: Message = bincode::deserialize(&update)
                            .expect("Failed to parse post mutation file");
                        let image = match message.has_image {
                            // true => self.image_store.get(&entry.uuid),
                            true => image::get(image_base_path, &entry.uuid),
                            false => None,
                        };
                        let complete_message = CompleteMessage::new(message, image);
                        result.posts.push(complete_message);
                    }
                    Kind::Put => {
                        let update =
                            std::fs::read(&path).expect("Failed to read put mutation file");
                        // do not use `bincode` here because it fails with the `Maybe` type
                        let update: PutUpdate = serde_json::from_slice(&update)
                            .expect("Failed to parse put mutation file");
                        let complete_update = CompletePutUpdate::new(update);
                        result.puts_deletes.push(PutDeleteUpdate {
                            put: Maybe::Value(complete_update),
                            delete: Maybe::Absent,
                        });
                    }
                    Kind::Delete => {
                        result.puts_deletes.push(PutDeleteUpdate {
                            put: Maybe::Absent,
                            delete: Maybe::Value(entry.uuid),
                        });
                    }
                }
            } else {
                // pagination is done
                result.done = true;
                let dir = self.mutation_dir.clone();
                tokio::spawn(async move {
                    MutationManager::clear_dir(&dir).ok();
                });
                break;
            }
        }

        result
    }

    pub fn clear(&mut self) {
        self.updates_post.clear();
        self.updates_put.clear();
        self.updates_delete.clear();
        self.updates_all.clear();
        MutationManager::clear_dir(&self.mutation_dir).ok();
    }

    fn clear_dir(dir: &PathBuf) -> std::io::Result<()> {
        // remove all files under mutation_dir
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    fn get_mutation_file_path(&self, uuid: &str) -> PathBuf {
        // mutation_dur/uuid
        std::path::Path::new(&self.mutation_dir).join(uuid)
    }
}
