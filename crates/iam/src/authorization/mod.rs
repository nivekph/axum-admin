mod engine;
mod error;
mod service;
mod store;

pub use error::AuthorizationError;
pub(crate) use error::{AccountPolicyError, RolePolicyError};
pub(crate) use service::{Authorization, ReplaceRoleAccess, ReplaceUserRoles};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectiveRoleGrant {
    pub id: i64,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectivePermissionGrant {
    pub permission: String,
    pub roles: Vec<EffectiveRoleGrant>,
}
