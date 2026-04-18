use atc_server::{build_router, AppState, ServerConfig};
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub struct TestServer {
    pub addr: SocketAddr,
    pub state: AppState,
    pub log_dir: Option<tempfile::TempDir>,
    _join: tokio::task::JoinHandle<()>,
}

impl TestServer {
    pub async fn start(mut cfg: ServerConfig) -> Self {
        let temp = if cfg.log_dir.is_none() {
            let dir = tempfile::tempdir().expect("tempdir");
            cfg.log_dir = Some(dir.path().to_path_buf());
            Some(dir)
        } else {
            None
        };
        let (router, state) = build_router(cfg);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let join = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Self {
            addr,
            state,
            log_dir: temp,
            _join: join,
        }
    }

    pub fn http_base(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn ws_base(&self) -> String {
        format!("ws://{}", self.addr)
    }
}
