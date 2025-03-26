use serde::Deserialize;
use serde::Deserializer;

#[derive(Copy, Clone)]
pub struct NonNegF32(f32);

impl<'de> Deserialize<'de> for NonNegF32 {
    fn deserialize<D>(deserializer: D) -> Result<NonNegF32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let res = f32::deserialize(deserializer)?;

        if res <= 0.0 {
            return Err(serde::de::Error::custom("second should not be negative"));
        }
        Ok(NonNegF32(res))
    }
}
