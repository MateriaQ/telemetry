use askama::Template;

pub const LIGHT_CSS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/public/css/light.css"));
pub const DARK_CSS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/public/css/dark.css"));
pub const MAIN_CSS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/public/css/main.css"));

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate<'a> {
    pub title: &'a str,
    pub current_page: &'a str,
    pub canonical_url: &'a str,
    pub meta_description: &'a str,
    pub light_css: &'a str,
    pub dark_css: &'a str,
    pub main_css: &'a str,
    pub apps: &'a [&'static crate::app::AppConfig],
}

#[derive(Template)]
#[template(path = "privacy.html")]
pub struct PrivacyTemplate<'a> {
    pub title: &'a str,
    pub current_page: &'a str,
    pub canonical_url: &'a str,
    pub meta_description: &'a str,
    pub light_css: &'a str,
    pub dark_css: &'a str,
    pub main_css: &'a str,
    pub custom_apps: &'a [crate::app::AppConfig],
}

#[derive(Template)]
#[template(path = "terms.html")]
pub struct TermsTemplate<'a> {
    pub title: &'a str,
    pub current_page: &'a str,
    pub canonical_url: &'a str,
    pub meta_description: &'a str,
    pub light_css: &'a str,
    pub dark_css: &'a str,
    pub main_css: &'a str,
    pub custom_apps: &'a [crate::app::AppConfig],
}

pub fn render_index() -> Result<String, askama::Error> {
    let all: Vec<&'static crate::app::AppConfig> = crate::app::all_apps().collect();
    IndexTemplate {
        title: "MateriaQ Telemetry - Privacy-Preserving Real-Time Analytics",
        current_page: "index",
        canonical_url: "https://telemetry.materiaq.org/",
        meta_description: "Real-time, privacy-first telemetry for Spicetify extensions. Zero personal data, no accounts, no IP tracking.",
        light_css: LIGHT_CSS,
        dark_css: DARK_CSS,
        main_css: MAIN_CSS,
        apps: &all,
    }
    .render()
}

pub fn render_privacy() -> Result<String, askama::Error> {
    PrivacyTemplate {
        title: "Privacy Policy - MateriaQ Telemetry",
        current_page: "privacy",
        canonical_url: "https://telemetry.materiaq.org/privacy",
        meta_description: "Privacy Policy for MateriaQ Telemetry: 100% anonymous, zero personal data, 4-minute presence, and no IP tracking.",
        light_css: LIGHT_CSS,
        dark_css: DARK_CSS,
        main_css: MAIN_CSS,
        custom_apps: crate::app::CUSTOM_APPS,
    }
    .render()
}

pub fn render_terms() -> Result<String, askama::Error> {
    TermsTemplate {
        title: "Terms of Service - MateriaQ Telemetry",
        current_page: "terms",
        canonical_url: "https://telemetry.materiaq.org/terms",
        meta_description: "Terms of Service for MateriaQ Telemetry: Acceptable usage, AGPLv3 licensing, and open-source terms.",
        light_css: LIGHT_CSS,
        dark_css: DARK_CSS,
        main_css: MAIN_CSS,
        custom_apps: crate::app::CUSTOM_APPS,
    }
    .render()
}
