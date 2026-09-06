use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};

use audit::{
    AuditAction, AuditContext, AuditEvent, AuditResource, AuditResult, AuditService, AuditValue,
    FieldChange,
};
use sqlx::PgPool;

use super::{
    AccountPolicyError, AuthorizationError, EffectivePermissionGrant, EffectiveRoleGrant,
    RolePolicyError,
    engine::EnforcementEngine,
    store::{PolicyStore, SUPER_ADMIN_CODE, normalize_ids},
};

#[derive(Debug, Clone)]
pub(crate) struct ReplaceUserRoles {
    pub actor_user_id: i64,
    pub user_id: i64,
    pub role_ids: Vec<i64>,
    pub audit_context: AuditContext,
}

#[derive(Debug, Clone)]
pub(crate) struct ReplaceRoleAccess {
    pub actor_user_id: i64,
    pub role_id: i64,
    pub permissions: BTreeSet<String>,
    pub audit_context: AuditContext,
}

#[derive(Clone)]
pub(crate) struct Authorization {
    store: Arc<PolicyStore>,
    engine: Arc<EnforcementEngine>,
    audits: AuditService,
}

impl Authorization {
    pub(crate) async fn load(pool: PgPool) -> Result<Self, AuthorizationError> {
        let store = Arc::new(PolicyStore::new(pool.clone()));
        let engine = Arc::new(EnforcementEngine::load(Arc::clone(&store)).await?);
        Ok(Self {
            store,
            engine,
            audits: AuditService::new(pool),
        })
    }

    pub(crate) async fn start_policy_sync(
        &self,
        redis_url: &str,
        reload_interval: Duration,
    ) -> Result<(), AuthorizationError> {
        self.engine.start_periodic_reload(reload_interval);
        self.engine.start_redis_watcher(redis_url).await
    }

    pub(crate) async fn user_status(&self, user_id: i64) -> Option<bool> {
        self.engine.user_status(user_id).await
    }

    pub(crate) async fn authorize_permission(
        &self,
        user_id: i64,
        permission: &str,
    ) -> Result<bool, AuthorizationError> {
        self.engine.authorize_permission(user_id, permission).await
    }

    pub(crate) async fn set_user_status(&self, user_id: i64, enabled: bool) {
        self.engine.set_user_status(user_id, enabled).await;
    }

    pub(crate) async fn set_role_status(&self, role_id: i64, enabled: bool) {
        self.engine.set_role_status(role_id, enabled).await;
    }

    pub(crate) fn notify_policy_changed(&self) {
        self.engine.notify_policy_changed();
    }

    pub(crate) async fn is_active_super_admin(
        &self,
        user_id: i64,
    ) -> Result<bool, AuthorizationError> {
        Ok(self.engine.is_active_super_admin(user_id).await)
    }

    pub(crate) async fn require_access_manager(
        &self,
        actor_user_id: i64,
        target_user_id: i64,
    ) -> Result<(), AccountPolicyError> {
        self.require_super_admin(actor_user_id).await?;
        if !self.store.user_exists(target_user_id).await? {
            return Err(AccountPolicyError::UserNotFound);
        }
        Ok(())
    }

    pub(crate) async fn require_role_manager(
        &self,
        actor_user_id: i64,
    ) -> Result<(), RolePolicyError> {
        if self
            .is_active_super_admin(actor_user_id)
            .await
            .map_err(RolePolicyError::Authorization)?
        {
            Ok(())
        } else {
            Err(RolePolicyError::AccessDenied)
        }
    }

    pub(crate) async fn require_mutable_role(
        &self,
        actor_user_id: i64,
        role_id: i64,
    ) -> Result<(), RolePolicyError> {
        self.require_role_manager(actor_user_id).await?;
        let role = self
            .store
            .role(role_id)
            .await?
            .ok_or(RolePolicyError::RoleNotFound)?;
        if role.code == SUPER_ADMIN_CODE {
            Err(RolePolicyError::RoleImmutable)
        } else {
            Ok(())
        }
    }

    pub(crate) async fn role_permissions(
        &self,
        role_id: i64,
    ) -> Result<Vec<String>, RolePolicyError> {
        if self.store.role(role_id).await?.is_none() {
            return Err(RolePolicyError::RoleNotFound);
        }
        Ok(self
            .engine
            .role_permissions(role_id)
            .await
            .into_iter()
            .collect())
    }

    pub(crate) async fn replace_role_access(
        &self,
        request: ReplaceRoleAccess,
    ) -> Result<(), RolePolicyError> {
        self.require_mutable_role(request.actor_user_id, request.role_id)
            .await?;
        let after = request.permissions.iter().cloned().collect::<Vec<_>>();
        let before = self
            .engine
            .replace_role_permissions(request.role_id, request.permissions)
            .await
            .map_err(RolePolicyError::Authorization)?;
        self.audits
            .record_best_effort(AuditEvent {
                req_id: request.audit_context.req_id,
                actor: request.audit_context.actor,
                action: AuditAction::ReplaceRoleAccess,
                resource: AuditResource::Role(request.role_id),
                result: AuditResult::Succeeded,
                reason_code: None,
                source: request.audit_context.source,
                changes: vec![FieldChange {
                    field: "permissions".to_string(),
                    before: AuditValue::Texts(before.into_iter().collect()),
                    after: AuditValue::Texts(after),
                }],
            })
            .await;
        Ok(())
    }

    pub(crate) async fn replace_user_roles(
        &self,
        request: ReplaceUserRoles,
    ) -> Result<(), AccountPolicyError> {
        self.require_super_admin(request.actor_user_id).await?;
        if !self.store.user_exists(request.user_id).await? {
            return Err(AccountPolicyError::UserNotFound);
        }
        let role_ids = normalize_ids(&request.role_ids);
        self.validate_role_replacement(request.user_id, &role_ids)
            .await?;
        let before = self
            .engine
            .replace_user_roles(request.user_id, role_ids.clone())
            .await?;
        self.audits
            .record_best_effort(AuditEvent {
                req_id: request.audit_context.req_id,
                actor: request.audit_context.actor,
                action: AuditAction::AssignUserRoles,
                resource: AuditResource::User(request.user_id),
                result: AuditResult::Succeeded,
                reason_code: None,
                source: request.audit_context.source,
                changes: vec![FieldChange {
                    field: "role_ids".to_string(),
                    before: AuditValue::Ids(before.into_iter().collect()),
                    after: AuditValue::Ids(role_ids.into_iter().collect()),
                }],
            })
            .await;
        Ok(())
    }

    pub(crate) async fn prepare_initial_user_roles(
        &self,
        actor_user_id: i64,
        role_ids: &[i64],
    ) -> Result<BTreeSet<i64>, AccountPolicyError> {
        let role_ids = normalize_ids(role_ids);
        if role_ids.is_empty() {
            return Ok(role_ids);
        }
        self.require_super_admin(actor_user_id).await?;
        self.validate_new_roles(&BTreeSet::new(), &role_ids).await?;
        Ok(role_ids)
    }

    pub(crate) async fn assign_initial_user_roles(
        &self,
        user_id: i64,
        role_ids: BTreeSet<i64>,
        audit_context: AuditContext,
    ) -> Result<(), AccountPolicyError> {
        if role_ids.is_empty() {
            return Ok(());
        }
        self.engine
            .replace_user_roles(user_id, role_ids.clone())
            .await?;
        self.audits
            .record_best_effort(AuditEvent {
                req_id: audit_context.req_id,
                actor: audit_context.actor,
                action: AuditAction::AssignUserRoles,
                resource: AuditResource::User(user_id),
                result: AuditResult::Succeeded,
                reason_code: None,
                source: audit_context.source,
                changes: vec![FieldChange {
                    field: "role_ids".to_string(),
                    before: AuditValue::Ids(Vec::new()),
                    after: AuditValue::Ids(role_ids.into_iter().collect()),
                }],
            })
            .await;
        Ok(())
    }

    pub(crate) async fn ensure_bootstrap_role(
        &self,
        user_id: i64,
        role_id: i64,
    ) -> Result<(), AuthorizationError> {
        let mut roles = self.engine.user_role_ids(user_id).await;
        roles.insert(role_id);
        self.engine.replace_user_roles(user_id, roles).await?;
        Ok(())
    }

    async fn validate_role_replacement(
        &self,
        user_id: i64,
        role_ids: &BTreeSet<i64>,
    ) -> Result<(), AccountPolicyError> {
        let current = self.engine.user_role_ids(user_id).await;
        self.validate_new_roles(&current, role_ids).await
    }

    async fn validate_new_roles(
        &self,
        current: &BTreeSet<i64>,
        requested: &BTreeSet<i64>,
    ) -> Result<(), AccountPolicyError> {
        let roles = self.store.roles(requested).await?;
        if roles.len() != requested.len()
            || requested
                .difference(current)
                .any(|id| roles.get(id).is_none_or(|role| role.status != "enabled"))
        {
            return Err(AccountPolicyError::InvalidRoleAssignment);
        }
        Ok(())
    }

    pub(crate) async fn user_role_ids(&self, user_id: i64) -> Result<Vec<i64>, AccountPolicyError> {
        if !self.store.user_exists(user_id).await? {
            return Err(AccountPolicyError::UserNotFound);
        }
        Ok(self
            .engine
            .user_role_ids(user_id)
            .await
            .into_iter()
            .collect())
    }

    pub(crate) async fn active_user_role_ids(
        &self,
        user_id: i64,
    ) -> Result<Vec<i64>, AuthorizationError> {
        Ok(self
            .active_user_role_ids_for(&[user_id])
            .await?
            .remove(&user_id)
            .unwrap_or_default())
    }

    pub(crate) async fn active_user_role_ids_for(
        &self,
        user_ids: &[i64],
    ) -> Result<HashMap<i64, Vec<i64>>, AuthorizationError> {
        let mut result = HashMap::new();
        for user_id in user_ids {
            result.insert(*user_id, self.engine.active_user_role_ids(*user_id).await);
        }
        Ok(result)
    }

    pub(crate) async fn permissions_for_roles(
        &self,
        active_role_ids: &[i64],
    ) -> Result<BTreeSet<String>, AuthorizationError> {
        let mut permissions = BTreeSet::new();
        for role_id in active_role_ids {
            permissions.extend(self.engine.role_permissions(*role_id).await);
        }
        Ok(permissions)
    }

    pub(crate) async fn effective_permission_grants(
        &self,
        active_role_ids: &[i64],
    ) -> Result<Vec<EffectivePermissionGrant>, AuthorizationError> {
        let ids = active_role_ids.iter().copied().collect::<BTreeSet<_>>();
        let roles = self.store.roles(&ids).await?;
        let mut grants = BTreeMap::<String, Vec<EffectiveRoleGrant>>::new();
        for role_id in ids {
            let Some(role) = roles.get(&role_id) else {
                continue;
            };
            for permission in self.engine.role_permissions(role_id).await {
                grants
                    .entry(permission)
                    .or_default()
                    .push(EffectiveRoleGrant {
                        id: role.id,
                        code: role.code.clone(),
                        name: role.name.clone(),
                    });
            }
        }
        Ok(grants
            .into_iter()
            .map(|(permission, roles)| EffectivePermissionGrant { permission, roles })
            .collect())
    }

    pub(crate) async fn role_has_members(&self, role_id: i64) -> bool {
        self.engine.role_has_members(role_id).await
    }

    pub(crate) async fn remove_user(&self, user_id: i64) -> Result<(), AuthorizationError> {
        self.engine.remove_user(user_id).await
    }

    pub(crate) async fn remove_role(&self, role_id: i64) -> Result<(), AuthorizationError> {
        self.engine.remove_role(role_id).await
    }

    async fn require_super_admin(&self, user_id: i64) -> Result<(), AccountPolicyError> {
        if self.is_active_super_admin(user_id).await? {
            Ok(())
        } else {
            Err(AccountPolicyError::AccessDenied)
        }
    }
}
