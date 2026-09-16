#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub source_code: Option<&'static str>,
    pub website: Option<&'static str>,
    pub aliases: &'static [&'static str],
    pub origins: &'static [&'static str],
}

pub const WEB_APP_ID: &str = "web";

pub const WEB_APP: AppConfig = AppConfig {
    id: WEB_APP_ID,
    name: "Telemetry Dashboard",
    description: "Live dashboard for active users and usage metrics",
    source_code: Some("https://github.com/MateriaQ/telemetry"),
    website: None,
    aliases: &["dashboard"],
    origins: &["https://telemetry.materiaq.org"],
};

pub const CUSTOM_APPS: &[AppConfig] = &[AppConfig {
    id: "lyrics",
    name: "MateriaQ Lyrics",
    description: "Customizable lyrics viewer for Spotify via Spicetify",
    source_code: Some("https://github.com/MateriaQ/lyrics"),
    website: Some("https://lyrics.materiaq.org/"),
    aliases: &["materiaq-lyrics"],
    origins: &[
        "https://lyrics.materiaq.org",
        "https://xpui.app.spotify.com",
    ],
}];

pub fn all_apps() -> impl Iterator<Item = &'static AppConfig> {
    CUSTOM_APPS.iter().chain(std::iter::once(&WEB_APP))
}

fn extract_origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let host_port = rest.split('/').next().unwrap_or(rest);
    if host_port.is_empty() {
        return None;
    }
    Some(format!("{scheme}://{host_port}"))
}

pub fn app_origins() -> Vec<String> {
    let mut origins = Vec::new();
    for app in all_apps() {
        for &o in app.origins {
            let clean = o.trim_end_matches('/').to_string();
            if !origins.contains(&clean) {
                origins.push(clean);
            }
        }
        if let Some(origin) = app
            .website
            .and_then(extract_origin)
            .filter(|o| !origins.contains(o))
        {
            origins.push(origin);
        }
    }
    origins
}

pub fn find_app(identifier: &str) -> Option<&'static AppConfig> {
    let needle = identifier.trim().to_lowercase();
    let unescaped = needle.replace("%20", " ").replace('+', " ");
    all_apps().find(|app| {
        app.id == needle
            || app.id == unescaped
            || app.name.to_lowercase() == needle
            || app.name.to_lowercase() == unescaped
            || app.aliases.iter().any(|&a| a == needle || a == unescaped)
    })
}

#[inline]
pub fn is_web(app_id: &str) -> bool {
    app_id.eq_ignore_ascii_case(WEB_APP_ID)
}

#[inline]
pub fn tracks_lifetime(app_id: &str) -> bool {
    !is_web(app_id)
}
