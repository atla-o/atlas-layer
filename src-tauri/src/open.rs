use crate::config::expand_tilde;

/// A link a desk item is allowed to hand to `xdg-open`.
/// `http` and `https` pages, or a file that exists on this machine.
pub fn external_target(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Nothing to open.".into());
    }
    if let Ok(url) = reqwest::Url::parse(raw) {
        return match url.scheme() {
            "http" | "https" => Ok(url.to_string()),
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| "That file URL is not on this machine.".to_string())?;
                if path.is_file() {
                    Ok(path.display().to_string())
                } else {
                    Err("That file is not on this machine.".into())
                }
            }
            _ => Err("Only web pages and local files can be opened.".into()),
        };
    }
    let path = expand_tilde(raw);
    if path.is_file() {
        Ok(path.display().to_string())
    } else {
        Err("That link is not a page or a file on this machine.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::external_target;

    #[test]
    fn web_pages_pass_and_other_schemes_do_not() {
        assert_eq!(
            external_target("https://github.com/atla-o/atlas-layer").unwrap(),
            "https://github.com/atla-o/atlas-layer"
        );
        assert!(external_target("javascript:alert(1)").is_err());
        assert!(external_target("").is_err());
        assert!(external_target("/tmp/this-atlas-file-should-not-exist").is_err());
    }
}
