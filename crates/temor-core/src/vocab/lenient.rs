//! Reading lists from stored files one item at a time: an item that does not parse costs only
//! itself, never the list it sits in.

use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(untagged)]
enum Item<T> {
    Kept(T),
    Dropped(IgnoredAny),
}

/// The items of a stored list that parse, in order.
pub(crate) fn lenient_list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
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
