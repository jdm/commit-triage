use std::sync::{LazyLock, OnceLock};

use comrak::markdown_to_html;
use rocket::{
    Config,
    fs::FileServer,
    futures::{SinkExt, StreamExt},
    get, routes,
    serde::json::Json,
    tokio::{
        self,
        sync::{Mutex, broadcast, mpsc, oneshot},
    },
};
use rocket_ws::Message;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    ARGS,
    analysis::WordCloud,
    commit::{Commit, State},
};

pub static SHUTDOWN: OnceLock<rocket::Shutdown> = OnceLock::new();

/// channel for notifying web clients that a content update is available.
///
/// contains a receiver to avoid SendError when there are no clients (`UPDATE.1`),
/// but you can’t call `recv` on it directly. instead create your own receiver
/// with `UPDATE.0.subscribe()` or `UPDATE.1.resubscribe()`.
static UPDATE: LazyLock<(broadcast::Sender<CommitExt>, broadcast::Receiver<CommitExt>)> =
    LazyLock::new(|| broadcast::channel(1));

pub static ACTION: LazyLock<(mpsc::Sender<Action>, Mutex<mpsc::Receiver<Action>>)> =
    LazyLock::new(|| {
        let (tx, rx) = mpsc::channel(1);
        (tx, rx.into())
    });

#[rocket::main]
pub async fn server() -> Result<(), Box<rocket::Error>> {
    let config = Config {
        port: ARGS.web_server_port.unwrap(),
        shutdown: rocket::config::Shutdown {
            grace: 0,
            mercy: 0,
            force: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let rocket = rocket::custom(&config)
        .mount("/", routes![ws, commits, word_cloud])
        .mount("/", FileServer::from("./static"))
        .ignite()
        .await?;
    SHUTDOWN.get_or_init(|| rocket.shutdown());

    rocket.launch().await?;
    Ok(())
}

pub fn shutdown() {
    crate::web::SHUTDOWN.get().unwrap().clone().notify();
}

impl From<Commit> for CommitExt {
    fn from(commit: Commit) -> Self {
        let rendered_body = safe_render_markdown(&commit.body.join("\n"));
        let git_show = if let Some(path) = ARGS.git_show_output_cache_path.as_ref() {
            std::fs::read_to_string(path.join(&commit.hash)).unwrap_or_else(|_| "".to_owned())
        } else {
            "[enable `git show` output with --git-show-cache-path]".to_owned()
        };
        let highfive_answers = if let Some(path) = ARGS.highfive_answers_path.as_ref() {
            std::fs::read_to_string(path.join(commit.number())).ok()
        } else {
            Some("[enable Highfive answers with --highfive-answers-path]".to_owned())
        };
        CommitExt {
            commit,
            rendered_body,
            git_show,
            rendered_highfive_answers: highfive_answers
                .map(|answers| safe_render_markdown(&answers)),
        }
    }
}

pub fn update(commit: &Commit) {
    UPDATE.0.send(CommitExt::from(commit.clone())).unwrap();
}

#[allow(clippy::let_and_return)]
fn safe_render_markdown(unsafe_markdown: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.autolink = true;
    options.extension.table = true;
    options.render.gfm_quirks = true;
    options.render.hardbreaks = true;
    options.render.r#unsafe = true;
    let unsafe_html = markdown_to_html(unsafe_markdown, &options);
    let safe_html = ammonia::clean(&unsafe_html);
    safe_html
}

#[get("/ws")]
fn ws(ws: rocket_ws::WebSocket) -> rocket_ws::Channel<'static> {
    let mut update = UPDATE.1.resubscribe();
    ws.channel(move |mut ws| {
        Box::pin(async move {
            loop {
                tokio::select! {
                    // we’ve updated a commit
                    update = update.recv() => {
                        let Ok(update) = update else {
                            error!(?update);
                            continue;
                        };
                        info!(?update, "commit updated");
                        // send the update
                        let update = serde_json::to_string(&update).expect("failed to convert update to JSON");
                        ws.send(update.into()).await?;
                    },

                    // the client sent a request
                    Some(message) = ws.next() => {
                        let Ok(Message::Text(request)) = message else {
                            error!(?message);
                            continue;
                        };
                        let request: Request = serde_json::from_str(&request).unwrap();
                        match request {
                            Request::SetLabel(commits, label) => {
                                info!(?label, ?commits, "setting label");
                                ACTION.0.send(Action::SetLabel(commits, label)).await.unwrap();
                            },
                            Request::SetState(commits, state) => {
                                info!(?state, ?commits, "setting state in bulk");
                                ACTION.0.send(Action::SetState(commits, state)).await.unwrap();
                            },
                        }
                    },
                };
            }
        })
    })
}

#[get("/commits")]
async fn commits() -> Json<Vec<CommitExt>> {
    info!("commits requested");
    let (tx, rx) = oneshot::channel();
    ACTION.0.send(Action::GetCommits(tx)).await.unwrap();
    rx.await
        .unwrap()
        .into_iter()
        .map(CommitExt::from)
        .collect::<Vec<_>>()
        .into()
}

#[get("/wordCloud")]
async fn word_cloud() -> Json<Result<WordCloud, &'static str>> {
    info!("word cloud requested");
    let (tx, rx) = oneshot::channel();
    ACTION.0.send(Action::GetWordCloud(tx)).await.unwrap();
    rx.await.unwrap().into()
}

pub enum Action {
    /// set the [`State`] of the each given [`Commit`].
    ///
    /// each `Commit` is looked up internally using its `hash_number`;
    /// all other fields are ignored.
    SetLabel(Vec<Commit>, String),
    /// set the [`State`] of the each given [`Commit`].
    ///
    /// each `Commit` is looked up internally using its `hash_number`;
    /// all other fields are ignored.
    SetState(Vec<Commit>, State),

    GetCommits(oneshot::Sender<Vec<Commit>>),
    GetWordCloud(oneshot::Sender<Result<WordCloud, &'static str>>),
}

#[derive(Clone, Debug, Serialize)]
struct CommitExt {
    #[serde(flatten)]
    commit: Commit,

    rendered_body: String,
    git_show: String,
    rendered_highfive_answers: Option<String>,
}

#[derive(Deserialize)]
enum Request {
    /// see [`Action::SetLabel`].
    SetLabel(Vec<Commit>, String),
    /// see [`Action::SetState`].
    SetState(Vec<Commit>, State),
}
