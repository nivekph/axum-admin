use serde::Deserialize;
use utoipa::IntoParams;

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ParamListQuery {
    pub page: Option<i64>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<i64>,
    pub name: Option<String>,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ParameterInput {
    pub name: String,
    pub key: String,
    pub value: String,
    pub desc: String,
}
