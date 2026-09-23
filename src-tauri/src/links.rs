use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

pub const SUPPORT_URL: &str = "https://buymeacoffee.com/luizfbalves";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    Support,
}

pub fn url(link: Link) -> &'static str {
    match link {
        Link::Support => SUPPORT_URL,
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
        let l: Link = serde_json::from_str("\"support\"").unwrap();
        assert_eq!(l, Link::Support);
    }
}
