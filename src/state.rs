use crate::token_bucket::IdentityCap;
use redis::aio::ConnectionManager;

#[derive(Clone)]
pub struct AppState {
    pub redis: ConnectionManager,
    pub admin_token: String,
    pub hmac_secret: Vec<u8>,
    pub index_html: String,
    pub terms_html: String,
    pub privacy_html: String,
    pub cookie_secure: bool,
    pub identity_cap: IdentityCap,
}
