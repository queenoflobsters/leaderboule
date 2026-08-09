use crate::api::UserPerformance;
use dioxus::prelude::*;

#[server]
pub async fn get_users_performances() -> Result<Vec<UserPerformance>> {
    let josephine = UserPerformance {
        name: "Josephine".into(),
        elo: 350,
        games_played: 23,
        games_won: 13,
    };
    let edward = UserPerformance {
        name: "Edward le sacro saint destructeur de chattes".into(),
        elo: 1000,
        games_played: 100,
        games_won: 100,
    };
    let pablo = UserPerformance {
        name: "Pablo".into(),
        elo: 8,
        games_played: 1331,
        games_won: 7,
    };
    let stella = UserPerformance {
        name: "Stella".into(),
        elo: 110,
        games_played: 1,
        games_won: 0,
    };
    Ok(vec![josephine, edward, pablo, stella])
}

