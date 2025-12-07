/// https://github.com/foundry-rs/foundry/blob/a06340288d45e6c4052efb22765dbeaa65a6f7a0/crates/anvil/core/src/eth/serde_helpers.rs
/// Licensed under Apache-2.0 OR MIT.  Copyright (c) 2021 Georgios
/// Konstantopoulos Copied and adapter for tlock-rs.
///
/// Directly copied, also includes the `common` module imported from:
/// https://github.com/foundry-rs/foundry/blob/a06340288d45e6c4052efb22765dbeaa65a6f7a0/crates/common/src/serde_helpers.rs

pub mod common {
    use alloy::primitives::U256;
    use serde::{Deserialize, Deserializer};

    /// Helper type to parse both `u64` and `U256`
    #[derive(Copy, Clone, Deserialize)]
    #[serde(untagged)]
    pub enum Numeric {
        /// A [U256] value.
        U256(U256),
        /// A `u64` value.
        Num(u64),
    }

    impl From<Numeric> for U256 {
        fn from(n: Numeric) -> Self {
            match n {
                Numeric::U256(n) => n,
                Numeric::Num(n) => Self::from(n),
            }
        }
    }

    /// Deserializes a number from hex or int
    pub fn deserialize_number<'de, D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        Numeric::deserialize(deserializer).map(Into::into)
    }
}

pub mod sequence {
    use serde::{Deserialize, Deserializer, de::DeserializeOwned};

    pub fn deserialize<'de, T, D>(d: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: DeserializeOwned,
    {
        let mut seq = Vec::<T>::deserialize(d)?;
        if seq.len() != 1 {
            return Err(serde::de::Error::custom(format!(
                "expected params sequence with length 1 but got {}",
                seq.len()
            )));
        }
        Ok(seq.remove(0))
    }
}

/// A module that deserializes `[]` optionally
pub mod empty_params {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(d: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        let seq = Option::<Vec<()>>::deserialize(d)?.unwrap_or_default();
        if !seq.is_empty() {
            return Err(serde::de::Error::custom(format!(
                "expected params sequence with length 0 but got {}",
                seq.len()
            )));
        }
        Ok(())
    }
}

/// A module that deserializes either a BlockNumberOrTag, or a simple number.
pub mod lenient_block_number {
    use alloy::eips::{BlockNumberOrTag, eip1898::LenientBlockNumberOrTag};
    use serde::{Deserialize, Deserializer};

    /// deserializes either a BlockNumberOrTag, or a simple number.
    pub use alloy::eips::eip1898::lenient_block_number_or_tag::deserialize as lenient_block_number;

    /// Same as `lenient_block_number` but requires to be `[num; 1]`
    pub fn lenient_block_number_seq<'de, D>(deserializer: D) -> Result<BlockNumberOrTag, D::Error>
    where
        D: Deserializer<'de>,
    {
        let num = <[LenientBlockNumberOrTag; 1]>::deserialize(deserializer)?[0].into();
        Ok(num)
    }
}
