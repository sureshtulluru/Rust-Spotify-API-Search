use reqwest;
use urlencoding;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Serialize, Deserialize, Debug)]
struct ExternalUrls {
    spotify: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Artist {
    name: String,
    external_urls: ExternalUrls,
}

#[derive(Serialize, Deserialize, Debug)]
struct Album {
    name: String,
    artists: Vec<Artist>,
    external_urls: ExternalUrls,
}

#[derive(Serialize, Deserialize, Debug)]
struct Track {
    name: String,
    album: Album,
    external_urls: ExternalUrls,
}

#[derive(Serialize, Deserialize, Debug)]
struct Items<T> {
    items: Vec<T>,
}

#[derive(Serialize, Deserialize, Debug)]
struct APIResponse {
    tracks: Items<Track>,
}

struct Database {
    connection: Connection,
}

impl Database {
    fn new() -> Result<Self, rusqlite::Error> {
        let connection = Connection::open("spotify.db")?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                album_name TEXT NOT NULL,
                artist_names TEXT NOT NULL,
                spotify_url TEXT NOT NULL
            )",
            [],
        )?; 
        Ok(Self { connection })
    }

    fn insert_track(&self, track: &Track) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "INSERT INTO tracks (name, album_name, artist_names, spotify_url) VALUES (?, ?, ?, ?)",
            [
                &track.name,
                &track.album.name,
                &track
                    .album
                    .artists
                    .iter()
                    .map(|artist| artist.name.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
                &track.external_urls.spotify,
            ],
        )?;
        Ok(())
    }

    fn query_tracks(&self) -> Result<(), rusqlite::Error> {
        let mut statement = self.connection.prepare("SELECT * FROM tracks")?;

        let track_iter = statement.query_map([], |row| {
            Ok(Track {
                name: row.get("name")?,
                album: Album {
                    name: row.get("album_name")?,
                    artists: row.get::<&str, String>("artist_names")?.split(", ").map(|name| Artist {

                        name: name.to_string(),
                        external_urls: ExternalUrls {
                            spotify: "".to_string(), // You may need to fetch this from the API again
                        },
                    }).collect(),
                    external_urls: ExternalUrls {
                        spotify: "".to_string(), // You may need to fetch this from the API again
                    },
                },
                external_urls: ExternalUrls {
                    spotify: row.get("spotify_url")?,
                },
            })
        })?;

        for track in track_iter {
            println!("{:?}", track?);
        }

        Ok(())
   }
 }

fn print_tracks(tracks: Vec<&Track>) {
    for track in tracks {
        println!("{}", track.name);
        println!("{}", track.album.name);
        println!(
            "{}",
            track
                .album
                .artists
                .iter()
                .map(|artist| artist.name.to_string())
                .collect::<String>()
        );
        println!("{}", track.external_urls.spotify);
        println!("---------");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let search_query = &args[1];
    let auth_token = &args[2];

    let encoded_query = urlencoding::encode(search_query);
    let url = format!(
        "https://api.spotify.com/v1/search?q={query}&type=track,artist",
        query = encoded_query
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", auth_token))
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .send()
        .await?;

    match response.status() {
        reqwest::StatusCode::OK => {
            match response.json::<APIResponse>().await {
                Ok(parsed) => {
                    let database = Database::new()?;
                    for track in parsed.tracks.items.iter() {
                        database.insert_track(track)?;
                    }
                    database.query_tracks()?; // Call the query function after inserting tracks
                    print_tracks(parsed.tracks.items.iter().collect());
                }
                Err(_) => println!("Hm, the response didn't match the shape we expected."),
            };
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            println!("Need to grab a new token");
        }
        other => {
            panic!("Uh oh! Something unexpected happened: {:?}", other);
        }
    };

    Ok(())
}



