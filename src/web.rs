use std::sync::{LazyLock, OnceLock, RwLock};

use comrak::markdown_to_html;
use rocket::{
    Config,
    fs::FileServer,
    futures::{SinkExt, StreamExt},
    get, routes,
    tokio::{
        self,
        sync::{Mutex, broadcast, mpsc},
    },
};
use rocket_ws::Message;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::commit::Commit;

pub static SHUTDOWN: OnceLock<rocket::Shutdown> = OnceLock::new();

/// channel for notifying web clients that a content update is available.
///
/// uses a tokio broadcast channel as a kind of async [`std::sync::Condvar`].
/// contains a receiver to avoid SendError when there are no clients (`UPDATE.1`),
/// but you can’t call `recv` on it directly. instead create your own receiver
/// with `UPDATE.0.subscribe()` or `UPDATE.1.resubscribe()`.
///
/// the content itself is stored separately, so we can send it on page load.
pub static UPDATE: LazyLock<(broadcast::Sender<()>, broadcast::Receiver<()>)> =
    LazyLock::new(|| broadcast::channel(1));
pub static CONTENT: RwLock<String> = RwLock::new(String::new());

pub static ACTION: LazyLock<(mpsc::Sender<Action>, Mutex<mpsc::Receiver<Action>>)> =
    LazyLock::new(|| {
        let (tx, rx) = mpsc::channel(1);
        (tx, rx.into())
    });

#[rocket::main]
pub async fn server() -> Result<(), rocket::Error> {
    let config = Config {
        shutdown: rocket::config::Shutdown {
            grace: 0,
            mercy: 0,
            force: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let rocket = rocket::custom(&config)
        .mount("/", routes![ws])
        .mount("/", FileServer::from("./static"))
        .ignite()
        .await?;
    SHUTDOWN.get_or_init(|| rocket.shutdown());

    rocket.launch().await?;
    Ok(())
}

pub fn update(commit: &Commit) {
    let mut options = comrak::Options::default();
    options.extension.autolink = true;
    options.extension.table = true;
    options.render.gfm_quirks = true;
    options.render.hardbreaks = true;
    options.render.r#unsafe = true;
    let unsafe_body = markdown_to_html(&commit.body.join("\n"), &options);
    let body = ammonia::clean(&unsafe_body);
    let content = Response {
        commit: commit.clone(),
        rendered_body: body,
    };
    *CONTENT.write().unwrap() = serde_json::to_string(&content).unwrap();
    UPDATE.0.send(()).unwrap();
}

#[get("/ws")]
fn ws(ws: rocket_ws::WebSocket) -> rocket_ws::Channel<'static> {
    let mut update = UPDATE.1.resubscribe();
    ws.channel(move |mut ws| {
        Box::pin(async move {
            // send the current content
            let content = CONTENT.read().unwrap().to_owned();
            ws.send(content.into()).await?;

            loop {
                tokio::select! {
                    // the content has changed
                    update = update.recv() => {
                        let Ok(()) = update else {
                            error!(?update);
                            continue;
                        };
                        info!("content changed");
                        // send the new content
                        let content = CONTENT.read().unwrap().to_owned();
                        ws.send(content.into()).await?;
                    },

                    // the client sent a request
                    Some(message) = ws.next() => {
                        let Ok(Message::Text(request)) = message else {
                            error!(?message);
                            continue;
                        };
                        let request: Request = serde_json::from_str(&request).unwrap();
                        match request {
                            Request::Keypress(key) => {
                                info!(?key, "key pressed");
                                ACTION.0.send(Action::Keypress(key)).await.unwrap();
                            },
                            Request::SetLabel(label) => {
                                info!(?label, "setting label");
                                ACTION.0.send(Action::SetLabel(label)).await.unwrap();
                            },
                            Request::Reload => {
                                info!("reload requested");
                                // send the current content
                                let content = CONTENT.read().unwrap().to_owned();
                                ws.send(content.into()).await?;
                            },
                        }
                    },
                };
            }
        })
    })
}

pub enum Action {
    Keypress(String),
    SetLabel(String),
}

#[derive(Serialize)]
struct Response {
    commit: Commit,
    rendered_body: String,
}

#[derive(Deserialize)]
enum Request {
    Keypress(String),
    SetLabel(String),
    Reload,
}
