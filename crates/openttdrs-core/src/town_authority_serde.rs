//! Serde de `Town::authority_ratings` con compatibilidad `local_authority_rating`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::town::{MAX_TOWN_AUTHORITY_COMPANIES, TOWN_RATING_INITIAL};

#[derive(Deserialize)]
#[serde(untagged)]
enum AuthorityRatingsCompat {
    PerCompany(Vec<i16>),
    LegacySingle(i16),
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<i16>, D::Error>
where
    D: Deserializer<'de>,
{
    let compat = AuthorityRatingsCompat::deserialize(deserializer)?;
    match compat {
        AuthorityRatingsCompat::PerCompany(v) if !v.is_empty() => Ok(v),
        AuthorityRatingsCompat::LegacySingle(rating) => {
            Ok(vec![rating; MAX_TOWN_AUTHORITY_COMPANIES])
        }
        AuthorityRatingsCompat::PerCompany(_) => {
            Ok(vec![TOWN_RATING_INITIAL; MAX_TOWN_AUTHORITY_COMPANIES])
        }
    }
}

pub fn serialize<S>(ratings: &Vec<i16>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    ratings.serialize(serializer)
}
