use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};

/// Transparent serde wrapper that allows trailing elements in arrays
#[derive(Debug, Clone)]
pub struct FlexArray<T>(pub T);

impl<T> Deref for FlexArray<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for FlexArray<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Serialize for FlexArray<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for FlexArray<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde_json::Value;

        let value = Value::deserialize(deserializer)?;

        // If it's an array, try truncating until it works
        if let Value::Array(mut arr) = value.clone() {
            while !arr.is_empty() {
                match serde_json::from_value(Value::Array(arr.clone())) {
                    Ok(result) => return Ok(FlexArray(result)),
                    Err(_) => {
                        arr.pop();
                    }
                }
            }
        }

        // Non-arrays or empty arrays
        serde_json::from_value(value)
            .map(FlexArray)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct TestStruct {
        a: i32,
        #[serde(default)]
        b: Option<String>,
    }

    #[test]
    fn test_serialize_personal_sign_params() {
        // Imagine we're in a function with params `data: Bytes, address: Address`
        let data = vec![1, 2, 3];
        let address: String = "0x1234567890abcdef1234567890abcdef12345678".into();

        // Code on the serialization side
        let json: String;
        {
            type Params = (Vec<i32>, String);
            let params: Params = (data.clone(), address.clone());
            let params = FlexArray(params);
            json = serde_json::to_string(&params).unwrap();
            assert_eq!(
                json,
                r#"[[1, 2, 3],"0x1234567890abcdef1234567890abcdef12345678"]"#
            );
        }

        // Code on the deserialization side
        {
            type Params = (Vec<i32>, String);
            let deserialized: FlexArray<Params> = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized.0.0, data);
            assert_eq!(deserialized.0.1, address);
        }
    }

    #[test]
    fn test_flex_array_deserialize_normal() {
        let json = r#"{"a": 42, "b": "hello"}"#;
        let result: FlexArray<TestStruct> = serde_json::from_str(json).unwrap();
        assert_eq!(result.a, 42);
        assert_eq!(result.b, Some("hello".into()));
    }

    #[test]
    fn test_flex_array_deserialize_positional() {
        let json = r#"[42, "hello"]"#;
        let result: FlexArray<TestStruct> = serde_json::from_str(json).unwrap();
        assert_eq!(result.a, 42);
        assert_eq!(result.b, Some("hello".into()));
    }

    #[test]
    fn test_flex_array_deserialize_missing() {
        let json = r#"[42]"#;
        let result: FlexArray<TestStruct> = serde_json::from_str(json).unwrap();
        assert_eq!(result.a, 42);
        assert_eq!(result.b, None);
    }

    #[test]
    fn test_flex_array_deserialize_extra() {
        let json = r#"[42, "hello", true, 3.14]"#;
        let result: FlexArray<TestStruct> = serde_json::from_str(json).unwrap();
        assert_eq!(result.a, 42);
        assert_eq!(result.b, Some("hello".into()));
    }

    //? Not sure if this is desirable behavior - it's unexpected. I originally
    //? wrote this test expecting it to fail, but it passes because the deserializer
    //? pops the invalid type off the end until it can successfully deserialize.
    //? I suppose this isn't bad behavior, but it could hide errors. Will
    //? leave it for now.
    #[test]
    fn test_invalid_optional_type() {
        let json = r#"[42, 100]"#; // b should be a string
        let result: FlexArray<TestStruct> = serde_json::from_str(json).unwrap();
        assert_eq!(result.a, 42);
        assert_eq!(result.b, None);
    }

    #[test]
    fn test_missing_required() {
        let json = r#"[]"#;
        let result: Result<FlexArray<TestStruct>, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_required_type() {
        let json = r#"["12", false]"#; // a should be a number
        let result: Result<FlexArray<TestStruct>, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
