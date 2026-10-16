//! Android stand-in for [`botguard`](botguard.rs): there is no V8 build for `rustypipe-botguard`
//! here, so no BotGuard runtime and no PoToken. Every spawn fails [`Error::Fatal`], which latches the
//! generator off (potoken/mod.rs) and leaves stream resolution to the direct-URL clients
//! (`innertube::STREAM_FALLBACK_ORDER`), the same graceful degradation the desktop build falls
//! back to when its runtime cannot start.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("botguard runtime: {0}")]
    Fatal(String),
    #[allow(dead_code)]
    #[error("botguard: {0}")]
    Transient(String),
}

pub struct Minter;

pub struct Bootstrap {
    pub minter: Minter,
    pub session_token: String,
    pub lifetime_secs: u64,
}

impl Minter {
    pub async fn spawn(_user_agent: String, _session_ident: String) -> Result<Bootstrap, Error> {
        Err(Error::Fatal("not available on Android".into()))
    }

    pub async fn mint(&self, _ident: &str) -> Result<String, Error> {
        Err(Error::Fatal("not available on Android".into()))
    }
}
