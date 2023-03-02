use serde::Serialize;

#[derive(Serialize)]
/// The model of the `messages` table.
pub struct Message {
    pub uuid: String,
    pub author: String,
    pub message: String,
    pub likes: i32,
    pub has_image: bool,
}

// pub struct MessageFields {
//     pub author: String,
//     pub message: String,
//     pub likes: i32,
//     pub has_image: bool,
// }
