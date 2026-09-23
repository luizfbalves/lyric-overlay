use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

pub const SUPPORT_URL: &str = "https://buymeacoffee.com/luizfbalves";
pub const DEEPL_SIGNUP_URL: &str = "https://www.deepl.com/pro-api";
pub const DEEPL_KEYS_URL: &str = "https://www.deepl.com/your-account/keys";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    Support,
    DeeplSignup,
    DeeplKeys,
}

pub fn url(link: Link) -> &'static str {
    match link {
        Link::Support => SUPPORT_URL,
        Link::DeeplSignup => DEEPL_SIGNUP_URL,
        Link::DeeplKeys => DEEPL_KEYS_URL,
    }
}

pub fn open(app: &AppHandle, link: Link) -> Result<(), String> {
    app.opener().open_url(url(link), None::<&str>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(url(Link::Support), "https://buymeacoffee.com/luizfbalves");
        assert_eq!(url(Link::DeeplSignup), "https://www.deepl.com/pro-api");
        assert_eq!(url(Link::DeeplKeys), "https://www.deepl.com/your-account/keys");
        let l: Link = serde_json::from_str("\"deepl_keys\"").unwrap();
        assert_eq!(l, Link::DeeplKeys);
    }
}
