use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    routing::get,
    AddExtensionLayer, Json, Router,
};
use futures::StreamExt;
use k8s_openapi::api::apps::v1::Deployment;
use kube::{
    runtime::{
        reflector,
        reflector::{ObjectRef, Store},
        utils::try_flatten_touched,
        watcher,
    },
    Api, Client, ResourceExt,
};
use serde::Serialize;
use std::net::SocketAddr;
use tokio::signal::unix::{signal, SignalKind};
use tower_http::trace::TraceLayer;
#[allow(unused_imports)]
use tracing::{debug, error, info, instrument, trace, warn};

type Result<T> = std::result::Result<T, anyhow::Error>;

#[derive(Serialize, Clone)]
pub struct Entry {
    container: String,
    name: String,
    namespace: String,
    version: String,
}
impl TryFrom<Deployment> for Entry {
    type Error = anyhow::Error;

    fn try_from(d: Deployment) -> Result<Self> {
        let name = d.name();
        let namespace = d.namespace().unwrap();
        if let Some(ref img) = d.spec.unwrap().template.spec.unwrap().containers[0].image {
            if img.contains(':') {
                let splits: Vec<_> = img.split(':').collect();
                return Ok(Entry {
                    name,
                    namespace,
                    container: splits[0].to_string(),
                    version: splits[1].to_string(),
                });
            }
        }
        Err(anyhow::anyhow!("Failed to parse deployment {}", name))
    }
}

// GET /versions
#[instrument(skip(store))]
async fn get_versions(store: Extension<Store<Deployment>>) -> Json<Vec<Entry>> {
    let state: Vec<Entry> = store
        .state()
        .into_iter()
        .filter_map(|d| Entry::try_from(d).ok())
        .collect();
    Json(state)
}

// GET /versions/<namespace>/<name>
#[instrument(skip(store))]
async fn get_version(
    store: Extension<Store<Deployment>>,
    Path((namespace, name)): Path<(String, String)>,
) -> std::result::Result<Json<Entry>, (StatusCode, &'static str)> {
    let key = ObjectRef::new(&name).within(&namespace);
    if let Some(d) = store.get(&key) {
        if let Ok(e) = Entry::try_from(d) {
            return Ok(Json(e));
        }
    }
    Err((StatusCode::NOT_FOUND, "not found"))
}

// GET /health
async fn health() -> (StatusCode, Json<&'static str>) {
    (StatusCode::OK, Json("healthy"))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    let client = Client::try_default().await.expect("create client");
    let api: Api<Deployment> = Api::default_namespaced(client);

    let store = reflector::store::Writer::<Deployment>::default();
    let reader = store.as_reader(); // queriable state for Axum
    let rf = reflector(store, watcher(api, Default::default()));
    // need to run/drain the reflector - so utilize the for_each to log deployment watch events
    let drainer = try_flatten_touched(rf)
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|o| {
            debug!("Touched {:?}", o.name());
            futures::future::ready(())
        });

    let app = Router::new()
        .route("/versions", get(get_versions))
        .route("/versions/:namespace/:name", get(get_version))
        .layer(AddExtensionLayer::new(reader.clone()))
        .layer(TraceLayer::new_for_http())
        // Reminder: routes added *after* TraceLayer are not subject to its logging behavior
        .route("/health", get(health));

    let mut shutdown = signal(SignalKind::terminate()).expect("could not monitor for SIGTERM");
    let server = axum::Server::bind(&SocketAddr::from(([0, 0, 0, 0], 8000)))
        .serve(app.into_make_service())
        .with_graceful_shutdown(async move {
            shutdown.recv().await;
        });

    tokio::select! {
        _ = drainer => warn!("reflector drained"),
        _ = server => info!("axum exited"),
    }
    Ok(())
}
