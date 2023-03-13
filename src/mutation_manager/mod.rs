use crate::handlers::{CompleteMessage, PaginationMetadata, PaginationType};
use ahash::AHashSet;
use serde::{Deserialize, Serialize};
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
    uuid: String,
    put: Option<ClientPutUpdate>,
    delete: bool,
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

#[derive(Serialize, Debug, Deserialize)]
/// The update that is saved to the mutation directory
pub struct ServerPutUpdate {
    pub author: String,
    pub message: String,
    pub likes: i32,
    pub image_updated: bool,
    pub image: Option<String>,
}

impl ServerPutUpdate {
    fn update(&mut self, other: ServerPutUpdate) {
        self.author = other.author;
        self.message = other.message;
        self.likes = other.likes;
        self.image_updated = other.image_updated || self.image_updated;
        if other.image_updated {
            self.image = other.image;
        };
    }
}

#[derive(Serialize, Debug, Deserialize)]
/// The update that the client sees.
pub struct ClientPutUpdate {
    pub author: String,
    pub message: String,
    pub likes: i32,
    pub image: Option<String>,
}

impl ClientPutUpdate {
    fn new(update: ServerPutUpdate) -> Self {
        let image = if update.image_updated {
            if let Some(image) = update.image {
                Some(image)
            } else {
                // image is removed
                Some("".to_string())
            }
        } else {
            None
        };
        Self {
            author: update.author,
            likes: update.likes,
            message: update.message,
            image,
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
            updates_post: AHashSet::with_capacity(50_000usize.next_power_of_two()),
            updates_put: AHashSet::with_capacity(10_000usize.next_power_of_two()),
            updates_delete: Vec::with_capacity(10_000usize.next_power_of_two()),
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

    pub fn add_put(&mut self, uuid: &str, put: ServerPutUpdate) {
        let path = self.get_mutation_file_path(uuid);

        // if there's a post update of this uuid, modify it rather than adding to updates_put
        if self.updates_post.contains(uuid) {
            // retrieve the message from the file
            let file_content = std::fs::read(&path).expect("Failed to read put mutation file");
            let mut message: CompleteMessage =
                bincode::deserialize(&file_content).expect("Failed to deserialize message");

            // overwrite the message with the new values
            message.update(put);

            // write back to the file
            let encoded = bincode::serialize(&message).unwrap();
            std::fs::write(&path, encoded).unwrap();
            return;
        }

        if self.updates_put.contains(uuid) {
            // retrieve the message from the file
            let file_content = std::fs::read(&path).expect("Failed to read put mutation file");
            let mut update: ServerPutUpdate =
                bincode::deserialize(&file_content).expect("Failed to deserialize message");
            update.update(put);
            // write back to the file
            let encoded = bincode::serialize(&update).unwrap();
            std::fs::write(&path, encoded).unwrap();
            return;
        }

        // create new file for this uuid
        let encoded = bincode::serialize(&put).unwrap();
        std::fs::write(path, encoded).unwrap();

        // add to updates_put
        self.updates_put.insert(uuid.to_string());
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

    pub fn get(&mut self) -> MutationResults {
        let mut result = MutationResults::default();

        // extract `page_size` updates from `updates_all` add them to `result`
        for _ in 0..self.page_size {
            if let Some(entry) = self.updates_all.pop_front() {
                let path = self.get_mutation_file_path(&entry.uuid);
                match entry.kind {
                    Kind::Post => {
                        let message =
                            std::fs::read(&path).expect("Failed to read post mutation file");
                        let message: CompleteMessage = bincode::deserialize(&message)
                            .expect("Failed to parse post mutation file");
                        result.posts.push(message);
                    }
                    Kind::Put => {
                        let server_update =
                            std::fs::read(&path).expect("Failed to read put mutation file");
                        let server_update: ServerPutUpdate = bincode::deserialize(&server_update)
                            .expect("Failed to parse put mutation file");
                        result.puts_deletes.push(PutDeleteUpdate {
                            uuid: entry.uuid,
                            put: Some(ClientPutUpdate::new(server_update)),
                            delete: false,
                        });
                    }
                    Kind::Delete => {
                        result.puts_deletes.push(PutDeleteUpdate {
                            uuid: entry.uuid,
                            put: None,
                            delete: true,
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
