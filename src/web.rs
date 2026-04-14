use std::sync::{LazyLock, OnceLock, RwLock};

use comrak::markdown_to_html;
use rocket::{
    Config,
    fs::FileServer,
    futures::SinkExt,
    get, routes,
    tokio::sync::broadcast::{Receiver, Sender, channel},
};

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
pub static UPDATE: LazyLock<(Sender<()>, Receiver<()>)> = LazyLock::new(|| channel(1));
pub static CONTENT: RwLock<String> = RwLock::new(String::new());

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
    let unsafe_content = markdown_to_html(&commit.body.join("\n"), &options);
    let content = ammonia::clean(&unsafe_content);
    *CONTENT.write().unwrap() = content;
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
                // wait for a notification that the content has changed
                update.recv().await.unwrap();

                // send the new content
                let content = CONTENT.read().unwrap().to_owned();
                ws.send(content.into()).await?;
            }
        })
    })
}
