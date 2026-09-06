use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    pub code: String,
    pub message: String,
    pub data: Option<T>,
}

/// Marker type for responses with no data payload (`data: null`).
///
/// Used as the `T` in `ApiResponse<T>` when there is nothing to return (errors,
/// logout, etc.). Callers pass `None` for `data`; this type is only for the type
/// system and OpenAPI schema.
#[derive(Debug, Serialize, ToSchema)]
pub struct EmptyData {}

impl<T> ApiResponse<T> {
    pub fn new(code: impl Into<String>, message: impl Into<String>, data: Option<T>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            data,
        }
    }

    pub fn ok(data: T) -> Self {
        Self::new("OK", "ok", Some(data))
    }
}

pub(crate) type ApiErrorResponse = ApiResponse<EmptyData>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_response_has_the_shared_null_data_contract() {
        let value = serde_json::to_value(ApiResponse::<EmptyData>::new("OK", "saved", None))
            .expect("mutation response should serialize");

        assert_eq!(value["code"], "OK");
        assert_eq!(value["message"], "saved");
        assert!(value["data"].is_null());
    }
}
