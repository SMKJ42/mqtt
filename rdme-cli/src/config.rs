use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Config {
    db_connection: String,
    // TODO: These are going to be stored in plaintext for now... needs to be handled properly.
    // terminal_username: String,
    // terminal_password: String,
}
