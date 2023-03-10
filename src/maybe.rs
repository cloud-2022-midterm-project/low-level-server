use serde::{ser::Error, Deserialize, Deserializer, Serialize, Serializer};

/// serde Value that can be Absent, Null, or Value(T)
#[derive(Debug, Default)]
pub enum Maybe<T> {
    #[default]
    Absent,
    Null,
    Value(T),
}

#[allow(dead_code)]
impl<T> Maybe<T> {
    pub fn is_absent(&self) -> bool {
        matches!(self, Maybe::Absent)
    }

    pub fn as_ref(&self) -> Maybe<&T> {
        match self {
            Maybe::Absent => Maybe::Absent,
            Maybe::Null => Maybe::Null,
            Maybe::Value(v) => Maybe::Value(v),
        }
    }
}

impl<T> From<Option<T>> for Maybe<T> {
    fn from(opt: Option<T>) -> Maybe<T> {
        match opt {
            Some(v) => Maybe::Value(v),
            None => Maybe::Null,
        }
    }
}

impl<'de, T> Deserialize<'de> for Maybe<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d = Option::deserialize(deserializer).map(Into::into);
        d
    }
}

impl<T: Serialize> Serialize for Maybe<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            // this will be serialized as null
            Maybe::Null => serializer.serialize_none(),
            Maybe::Value(v) => v.serialize(serializer),
            // should have been skipped
            Maybe::Absent => Err(Error::custom(
                r#"Maybe fields need to be annotated with: #[serde(default, skip_serializing_if = "Maybe::is_Absent")]"#,
            )),
        }
    }
}
