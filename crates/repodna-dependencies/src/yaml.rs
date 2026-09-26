//! Guarded YAML loading.

use yaml_rust2::{Yaml, YamlLoader};

/// Largest YAML document parsed (2 MiB).
const MAX_YAML_BYTES: usize = 2 * 1024 * 1024;
/// Maximum anchors plus aliases, a guard against alias-expansion ("billion laughs") attacks.
const MAX_ANCHORS: usize = 256;

/// Parses the first document of `text`, refusing oversized input and heavy alias use.
pub fn load(text: &str) -> Result<Yaml, String> {
    if text.len() > MAX_YAML_BYTES {
        return Err(format!("YAML document larger than {MAX_YAML_BYTES} bytes"));
    }
    let anchors = text
        .as_bytes()
        .windows(2)
        .filter(|pair| {
            matches!(pair[0], b'&' | b'*') && (pair[1].is_ascii_alphanumeric() || pair[1] == b'_')
        })
        .count();
    if anchors > MAX_ANCHORS {
        return Err("YAML document uses too many anchors or aliases".to_owned());
    }
    let mut documents = YamlLoader::load_from_str(text).map_err(|error| error.to_string())?;
    if documents.is_empty() {
        return Ok(Yaml::Null);
    }
    Ok(documents.swap_remove(0))
}

/// Returns a string or number value as text.
pub fn scalar(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(text) | Yaml::Real(text) => Some(text.clone()),
        Yaml::Integer(number) => Some(number.to_string()),
        Yaml::Boolean(flag) => Some(flag.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_documents_and_scalars() {
        let doc = load("name: demo\nversion: 1.2\ncount: 3\n").unwrap();
        assert_eq!(scalar(&doc["name"]).as_deref(), Some("demo"));
        assert_eq!(scalar(&doc["version"]).as_deref(), Some("1.2"));
        assert_eq!(scalar(&doc["count"]).as_deref(), Some("3"));
        assert_eq!(load("").unwrap(), Yaml::Null);
        assert!(load("key: [unclosed").is_err());
    }

    #[test]
    fn refuses_alias_bombs() {
        let mut bomb = String::from("a: &a [\"x\"]\n");
        for index in 0..300 {
            bomb.push_str(&format!("k{index}: *a\n"));
        }
        assert!(load(&bomb).unwrap_err().contains("anchors"));
    }
}
