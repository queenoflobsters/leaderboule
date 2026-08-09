use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct UserPerformance {
    pub name: String, // maybe later use the Rc<str> thing ?
    pub elo: u32,
    pub games_played: u32,
    pub games_won: u32
}
