use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Metadata is a typed wrapper over `serde_json::Value::Object`.
/// SEELE doesn't enforce schema — consumers (MNEMA, etc) define their own.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Metadata(pub Value);

impl Metadata {
    pub fn new() -> Self {
        Self(Value::Object(Map::new()))
    }

    pub fn from_value(v: Value) -> Self {
        Self(v)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn set(&mut self, key: &str, value: Value) {
        if let Value::Object(map) = &mut self.0 {
            map.insert(key.to_string(), value);
        }
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Value> for Metadata {
    fn from(v: Value) -> Self {
        Self(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty_object() {
        let m = Metadata::new();
        assert_eq!(m.as_str(), "{}");
    }

    #[test]
    fn set_and_get() {
        let mut m = Metadata::new();
        m.set("kind", Value::String("decision".into()));
        assert_eq!(m.get("kind").unwrap().as_str().unwrap(), "decision");
    }

    #[test]
    fn roundtrip_serde() {
        let mut m = Metadata::new();
        m.set("kind", Value::String("verdict".into()));
        m.set("axiomatic", Value::Bool(true));
        let s = serde_json::to_string(&m).unwrap();
        let m2: Metadata = serde_json::from_str(&s).unwrap();
        assert_eq!(m.as_str(), m2.as_str());
    }
}
