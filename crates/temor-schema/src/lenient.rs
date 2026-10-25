//! Reading stored files leniently: a list item or a field that does not parse costs only itself,
//! never the list, the struct or the file it sits in.

use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(untagged)]
enum Item<T> {
    Kept(T),
    Dropped(IgnoredAny),
}

/// The items of a stored list that parse, in order.
pub fn lenient_list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let items = Vec::<Item<T>>::deserialize(deserializer)?;
    Ok(items
        .into_iter()
        .filter_map(|item| match item {
            Item::Kept(value) => Some(value),
            Item::Dropped(_) => None,
        })
        .collect())
}

/// A stored value, or its default when it does not parse: one bad field costs only itself.
pub fn lenient_or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(match Item::<T>::deserialize(deserializer)? {
        Item::Kept(value) => value,
        Item::Dropped(_) => T::default(),
    })
}
