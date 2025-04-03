use std::time::Duration;

use serde::Deserialize;
use serde::Deserializer;

/// `std::time::Duration` expect to have two field, `secs` and `nanos`, which is a bit inconvienient to write in the TOML file
/// Instead, this Duration expect to have just one field: the number of seconds, as a f32
#[derive(Copy, Clone)]
pub struct DurationDeserF32(Duration);

impl<'de> Deserialize<'de> for DurationDeserF32 {
    fn deserialize<D>(deserializer: D) -> Result<DurationDeserF32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let res = f32::deserialize(deserializer)?;

        match Duration::try_from_secs_f32(res) {
            Ok(d) => Ok(DurationDeserF32(d)),
            Err(e) => Err(serde::de::Error::custom(e)),
        }
    }
}

impl Into<Duration> for DurationDeserF32 {
    fn into(self) -> Duration {
        self.0
    }
}
