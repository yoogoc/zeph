use crate::types::LocaleTranslations;
use serde_yaml::Value;
use std::collections::HashMap;

/// Flatten nested YAML structure into dot-notation keys
///
/// # Example
/// ```
/// # use i18n::yaml_utils::flatten_yaml;
/// # use serde_yaml::Value;
/// let yaml = r#"
/// common:
///   save: "Save"
///   cancel: "Cancel"
/// "#;
/// let value: Value = serde_yaml::from_str(yaml).unwrap();
/// let flattened = flatten_yaml(&value);
/// assert_eq!(flattened.get("common.save"), Some(&"Save".to_string()));
/// assert_eq!(flattened.get("common.cancel"), Some(&"Cancel".to_string()));
/// ```
pub fn flatten_yaml(value: &Value) -> LocaleTranslations {
    let mut result = HashMap::new();
    flatten_recursive(value, String::new(), &mut result);
    result
}

fn flatten_recursive(value: &Value, prefix: String, result: &mut LocaleTranslations) {
    match value {
        Value::Mapping(map) => {
            for (key, val) in map {
                // Skip the _version metadata field
                if let Value::String(key_str) = key {
                    if key_str == "_version" {
                        continue;
                    }

                    let new_prefix = if prefix.is_empty() {
                        key_str.clone()
                    } else {
                        format!("{}.{}", prefix, key_str)
                    };

                    flatten_recursive(val, new_prefix, result);
                }
            }
        }
        Value::String(s) => {
            result.insert(prefix, s.clone());
        }
        Value::Number(n) => {
            result.insert(prefix, n.to_string());
        }
        Value::Bool(b) => {
            result.insert(prefix, b.to_string());
        }
        _ => {
            // Skip null, sequences, and other types
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatten_simple() {
        let yaml = r#"
common:
  save: "Save"
  cancel: "Cancel"
"#;
        let value: Value = serde_yaml::from_str(yaml).unwrap();
        let flattened = flatten_yaml(&value);

        assert_eq!(flattened.get("common.save"), Some(&"Save".to_string()));
        assert_eq!(flattened.get("common.cancel"), Some(&"Cancel".to_string()));
    }

    #[test]
    fn test_flatten_nested() {
        let yaml = r#"
folders:
  fields:
    name: "Name"
    description: "Description"
"#;
        let value: Value = serde_yaml::from_str(yaml).unwrap();
        let flattened = flatten_yaml(&value);

        assert_eq!(
            flattened.get("folders.fields.name"),
            Some(&"Name".to_string())
        );
        assert_eq!(
            flattened.get("folders.fields.description"),
            Some(&"Description".to_string())
        );
    }

    #[test]
    fn test_flatten_skips_version() {
        let yaml = r#"
_version: 2
common:
  save: "Save"
"#;
        let value: Value = serde_yaml::from_str(yaml).unwrap();
        let flattened = flatten_yaml(&value);

        assert!(!flattened.contains_key("_version"));
        assert_eq!(flattened.get("common.save"), Some(&"Save".to_string()));
    }

    #[test]
    fn test_flatten_empty() {
        let yaml = r#"{}"#;
        let value: Value = serde_yaml::from_str(yaml).unwrap();
        let flattened = flatten_yaml(&value);

        assert!(flattened.is_empty());
    }

    #[test]
    fn test_flatten_preserves_special_chars() {
        let yaml = r#"
message: "Hello %{name}!"
url: "http://example.com"
"#;
        let value: Value = serde_yaml::from_str(yaml).unwrap();
        let flattened = flatten_yaml(&value);

        assert_eq!(
            flattened.get("message"),
            Some(&"Hello %{name}!".to_string())
        );
        assert_eq!(
            flattened.get("url"),
            Some(&"http://example.com".to_string())
        );
    }
}
