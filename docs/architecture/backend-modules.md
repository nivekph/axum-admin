# Backend Module Architecture

This document defines the backend module organization and dependency rules for `axum-admin`.

It is a module and responsibility contract, not a requirement that every feature use the same directory template.

The goal is to keep capability boundaries clear as the backend grows, while avoiding ceremonial abstraction, duplicated DTOs, catch-all service files, and persistence logic leaking into transport code.

The guiding principle is:

> Organize code by capability and responsibility, not by technical layer or directory symmetry.

A good module structure should make it obvious:

1. where business rules live;
2. where persistence logic lives;
3. who owns transactions;
4. where authorization is enforced;
5. where HTTP-specific behavior lives;
6. where domain errors become public API errors;
7. which types are public capability APIs and which are implementation details.

---

# 1. Scope and Authority

This document is the canonical contract for backend module organization.

It defines:

* crate responsibility boundaries;
* dependency direction;
* transport vs capability responsibilities;
* repository / store / backend semantics;
* transaction ownership;
* module visibility;
* feature growth and split rules;
* migration constraints.

It does not replace domain-specific architecture documents.

More specific documents remain authoritative for their own concerns.

## DTO ownership

API/capability DTO ownership is defined by:

```text
docs/architecture/api-dto-ownership.md
```

This document does not redefine that contract.

In particular, do not duplicate field-for-field identical API and capability request types merely for layer purity.

---

## IAM semantics

IAM policy, permission, role, audit, reload, and authorization semantics are defined by:

```text
docs/architecture/iam.md
```

This document defines how IAM modules should be organized, but does not redefine IAM behavior.

---

## AGENTS.md

`AGENTS.md` provides operational guidance and links to architecture contracts.

It must not become a second independent source of architecture truth.

When this document changes a durable architecture rule, update `AGENTS.md` to point to the new location rather than duplicating large sections of the rule.

---

# 2. Top-Level Backend Structure

The current crate boundaries are retained.

```text
crates/
├── api/
├── audit/
├── auth/
├── db/
├── file-storage/
├── iam/
└── metadata/
```

These boundaries already represent meaningful capabilities or infrastructure concerns.

Do not reorganize the backend into global technical-layer crates such as:

```text
domain/
services/
repositories/
controllers/
models/
```

That would spread one business capability across unrelated global directories.

The backend remains capability-first.

---

# 3. Crate Responsibilities

## `crates/api`

Owns HTTP transport concerns:

* Axum route registration;
* request extraction;
* middleware;
* HTTP request DTOs when the wire shape differs from capability input;
* HTTP response DTOs;
* OpenAPI;
* HTTP error representation;
* router/server composition;
* application HTTP state;
* multi-capability HTTP workflows when no single capability naturally owns the workflow.

It must not own reusable domain policy.

If a rule represents reusable domain policy rather than transport sequencing, move it into the owning capability. Reuse across multiple HTTP entry points is a strong signal that the rule does not belong in `api`.

---

## `crates/auth`

Owns authentication primitives and session lifecycle:

* password hashing and verification;
* captcha;
* JWT;
* login sessions;
* access-token validation;
* refresh-token lifecycle;
* session persistence.

It does not own user metadata, role policy, or account administration.

---

## `crates/iam`

Owns identity and authorization capabilities:

* Accounts;
* Roles;
* Departments;
* Menus;
* Access Catalog;
* Authorization policy and enforcement.

---

## `crates/metadata`

Owns application-managed metadata:

* Dictionaries;
* Dictionary detail trees;
* Parameters.

---

## `crates/file-storage`

Owns file and storage lifecycle:

* uploaded file metadata;
* object access;
* upload sessions;
* upload recovery;
* storage backend configuration;
* local/S3 object storage behavior.

---

## `crates/audit`

Owns:

* audit events;
* audit persistence;
* audit analysis.

---

## `crates/db`

Owns database infrastructure only.

It may own:

* pool initialization;
* migration infrastructure;
* generic DB bootstrap.

It must not become the place where domain SQL is centralized.

For example:

```text
iam/accounts/repository.rs
```

owns account persistence.

Do not move that logic into:

```text
db/accounts.rs
```

merely because it accesses PostgreSQL.

---

# 4. Dependency Direction

The normal backend request flow is:

```text
HTTP
 ↓
API transport
 ↓
Capability / use case
 ↓
Persistence / external adapter
 ↓
PostgreSQL / Redis / Casbin / OpenDAL
```

Conceptually:

```text
┌─────────────────────────────┐
│ API / Transport             │
│ routes, DTOs, HTTP errors   │
└──────────────┬──────────────┘
               ↓
┌─────────────────────────────┐
│ Capability / Use Case       │
│ business rules, policy      │
│ orchestration, transaction  │
└──────────────┬──────────────┘
               ↓
┌─────────────────────────────┐
│ Persistence / Adapter       │
│ repository/store/backend    │
└──────────────┬──────────────┘
               ↓
      PostgreSQL / Redis /
       Casbin / OpenDAL
```

Allowed dependencies include:

```text
api -> iam
api -> auth
api -> metadata
api -> file-storage
api -> audit

iam -> audit
capability -> infrastructure dependency
```

Disallowed dependency direction:

```text
iam -> api
auth -> api
metadata -> api
file-storage -> api

repository -> api
store -> api
backend -> api
```

Capability crates must remain independent of HTTP.

---

# 5. API as a Transport Adapter

`crates/api` translates between HTTP and backend capabilities.

Its primary role is:

```text
HTTP input
    ↓
capability call
    ↓
HTTP output
```

It should not reproduce capability business rules.

Typical route responsibilities:

```text
extract request
authenticate/admit request
convert transport input if needed
call capability
convert capability output
return HTTP response
```

---

# 6. Multi-Capability HTTP Workflows

Not every workflow belongs cleanly to one capability.

Login and refresh are important examples.

A login workflow may coordinate:

```text
captcha
   ↓
Accounts
   ↓
password verification
   ↓
TokenService
   ↓
HTTP response
```

No single existing capability naturally owns that whole workflow.

Therefore:

> A multi-capability workflow with no natural capability owner may remain in `api/routes/<feature>`.

The route may:

* sequence capability calls;
* preserve workflow ordering;
* apply transport-specific error semantics;
* build the final HTTP response.

The route must not own reusable capability policy.

For example, the API layer must not independently implement:

```text
department scope rules
role validity rules
account authorization rules
session persistence semantics
IAM policy rules
```

Those remain in the owning capability.

The distinction is:

```text
API workflow
    coordinates capabilities

Capability
    owns reusable business rules
```

Do not introduce a new `application` crate solely to move a small number of multi-capability HTTP workflows out of `api`.

---

# 7. API Top-Level Target Structure

The target API structure is:

```text
api/src/
├── error/
│   ├── mod.rs
│   ├── transport.rs
│   ├── auth.rs
│   ├── iam.rs
│   ├── metadata.rs
│   ├── storage.rs
│   └── audit.rs
│
├── extractors/
├── middleware/
├── routes/
│
├── docs.rs
├── request_id.rs
├── response.rs
├── router.rs
├── server.rs
├── state.rs
└── lib.rs
```

---

# 8. HTTP Response Ownership

`response.rs` remains intentionally small.

It owns the shared response envelope.

For example:

```rust
pub struct ApiResponse<T> {
    pub code: String,
    pub message: String,
    pub data: Option<T>,
}

pub struct EmptyData;
```

Its responsibility is:

> What does an HTTP response body look like?

Do not turn `response.rs` into a general mapping or presentation-layer directory.

Capability error conversion does not belong in `response.rs`.

---

# 9. HTTP Error Ownership

HTTP error behavior belongs under:

```text
api/src/error/
```

This is the API error boundary.

It translates:

```text
capability failure
    ↓
public HTTP error contract
```

---

## `error/mod.rs`

Owns generic API error machinery:

```text
AppError
AppResult
ErrorKind
ErrorSpec
IntoResponse
JsonRejection handling
serde_json rejection handling
generic internal errors
public error sanitization
```

It should contain infrastructure shared by all API error mappings.

Generic contracts such as:

```text
INTERNAL_SERVER_ERROR
```

may live here.

---

## `error/transport.rs`

Owns stable errors belonging to transport or middleware infrastructure rather than a backend capability.

Examples:

```text
RATE_LIMITED
RATE_LIMIT_UNAVAILABLE
```

It may also contain other genuinely transport-owned stable errors when they emerge.

Do not create a generic `misc.rs` bucket.

If an error has a real capability owner, place it with that owner.

---

## `error/auth.rs`

Owns stable HTTP mappings for auth errors whose public meaning is context-independent.

Examples:

```text
TokenIssueError -> AppError
TokenSessionError -> AppError
RefreshError -> AppError
TokenRevokeError -> AppError
```

Workflow-specific contracts such as `INVALID_CREDENTIALS` may remain route-local when multiple underlying failures intentionally collapse into one public meaning. A shared `ErrorSpec` constant does not imply a global `From` implementation.

---

## `error/iam.rs`

Owns HTTP mappings for IAM errors.

Examples:

```text
AccountError
RoleError
DeptError
AuthorizationError
AccessEvaluationError
```

and stable public contracts such as:

```text
USER_NOT_FOUND
USER_ALREADY_EXISTS
ROLE_NOT_FOUND
ROLE_IMMUTABLE
PERMISSION_DENIED
INVALID_ROLES
```

---

## `error/metadata.rs`

Owns HTTP mappings for:

```text
DictionaryError
ParameterError
```

---

## `error/storage.rs`

Owns HTTP mappings for:

```text
FileError
StorageError
```

---

## `error/audit.rs`

Owns HTTP mappings for:

```text
AuditError
AuditAnalysisError
```

This includes analysis/provider contracts such as:

```text
AI_PROVIDER_UNAVAILABLE
AI_RESPONSE_INVALID
```

These errors belong to `audit` because `AuditAnalysisError` owns the capability semantics.

Do not create a separate API error capability merely because the public code contains the word `AI`.

Ownership follows the backend capability, not superficial terminology.

---

# 10. Domain Errors vs HTTP Errors

Capability errors describe what happened in the domain.

Example:

```rust
pub enum AccountError {
    NotFound,
    AlreadyExists,
    AccessDenied,
    Database(sqlx::Error),
}
```

Capability errors must not encode HTTP behavior such as:

```text
404
403
409
HTTP response bodies
public error codes
```

The API layer decides how those errors appear to clients.

The separation is:

```text
capability/error.rs
    what failed

api/error/*.rs
    how that failure appears over HTTP
```

---

# 11. Stable vs Context-Specific Error Mapping

Add:

```rust
impl From<SomeError> for AppError
```

only when that error has one stable HTTP meaning in every relevant context.

If the same source error means different things in different workflows, map it explicitly at the call site.

Example:

```text
unknown login username
wrong login password
```

may intentionally collapse into:

```text
INVALID_CREDENTIALS
```

even though account-management lookup semantics elsewhere might be:

```text
USER_NOT_FOUND
```

Do not force every private error through a global `From` mapping if workflow context changes its public meaning.

---

# 12. API Route Organization

Routes remain feature-oriented.

Good:

```text
routes/
├── auth/
├── users/
├── roles/
├── departments/
├── dictionaries/
├── parameters/
├── files/
├── storages/
└── audit/
```

Do not reorganize the API into global technical directories such as:

```text
handlers/
controllers/
requests/
responses/
```

That would spread one feature across unrelated locations.

---

# 13. API Feature Structure

A simple HTTP feature may use:

```text
users/
├── mod.rs
├── dto.rs
└── handler.rs
```

This remains valid while the feature is cohesive.

When a route feature develops clearly distinct endpoint groups, it may later split by responsibility.

For example:

```text
dictionaries/
├── mod.rs
├── dto.rs
├── catalog.rs
└── tree.rs
```

However:

> Capability structure should be stabilized before mirroring the same split in API routes.

During the dictionaries capability refactor, keep the existing API route structure unchanged.

Do not split capability and transport modules in the same PR unless there is a concrete dependency requiring both.

---

# 14. Capability-First Organization

Capability crates remain feature-first.

For example:

```text
iam/src/
├── access/
├── accounts/
├── authorization/
├── departments/
├── menus/
└── roles/
```

and:

```text
metadata/src/
├── dictionaries/
└── parameters/
```

Do not reorganize a capability crate into:

```text
models/
repositories/
services/
errors/
```

at the crate level.

Technical responsibilities belong inside the owning feature.

---

# 15. Standard Simple Feature Shape

A typical SQL-backed feature may use:

```text
feature/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
└── service.rs
```

This is a common shape, not a mandatory template.

Files should exist only when the corresponding responsibility exists.

---

# 16. `mod.rs`

`mod.rs` owns:

```text
module declarations
public exports
primary facade definition when appropriate
very small core types
```

Typical example:

```rust
mod error;
mod model;
mod repository;
mod request;
mod service;

pub use error::ParameterError;
pub use model::*;
pub use request::*;
pub use service::ParameterService;
```

Avoid placing substantial SQL or workflow logic in `mod.rs`.

---

# 17. `error.rs`

Owns capability/domain errors.

Examples:

```text
NotFound
AlreadyExists
InvalidParent
AccessDenied
Database
```

These errors express capability semantics.

They must not depend on API types or HTTP concepts.

---

# 18. `model.rs`

Owns domain and read models.

Examples:

```text
UserInfoView
RoleSummary
DictionaryWithDetails
StorageView
```

Persistence-specific row types may also live here when small and tightly coupled to the model.

If database row representations become large or purely persistence-specific, keep them private to `repository.rs` instead.

---

# 19. `request.rs`

Owns capability inputs and queries.

Examples:

```text
UserListQuery
CreateAccountInput
UpdateCurrentUserInput
DictionaryInput
DictionaryDetailInput
ParamListQuery
```

Do not split into:

```text
query.rs
command.rs
input.rs
```

merely for architectural symmetry.

Meaningful type names matter more than file-name purity.

DTO ownership itself remains governed by:

```text
docs/architecture/api-dto-ownership.md
```

---

# 20. Repository Semantics

`repository.rs` is the default name for relational persistence owned by a feature.

Its responsibilities may include:

```text
SQL
database reads
database writes
row mapping
persistence-specific existence checks
database query helpers
```

Example:

```rust
pub(super) async fn find(
    pool: &PgPool,
    id: i64,
) -> Result<Option<SysDictionary>, sqlx::Error> {
    // SQL
}
```

---

# 21. Repository Must Not Own Business Policy

Do not place these concerns in repositories:

```text
authorization
HTTP mapping
audit policy
business access rules
cross-capability orchestration
workflow decisions
```

Bad:

```rust
if !actor_is_admin {
    return Err(AccountError::AccessDenied);
}
```

inside `repository.rs`.

Good:

```rust
let user = repository::find_user(...).await?;

if !self.can_manage(actor_user_id, &user).await? {
    return Err(AccountError::AccessDenied);
}
```

inside a use-case module.

---

# 22. Repository Visibility

Repositories are implementation details.

The repository module itself should normally remain private:

```rust
mod repository;
```

Repository functions should use the narrowest visibility that satisfies real callers.

Preferred order:

```text
private
pub(super)
pub(in crate::<feature>)
pub(crate)
```

Use `pub(crate)` only when there is a real cross-feature requirement.

Do not expose repositories outside the crate.

The public API of a capability should be its facade and domain types, for example:

```text
Accounts
DictionaryService
RoleService
ParameterService
```

not:

```text
AccountRepository
DictionaryRepository
```

unless repository exposure becomes an explicit architecture decision.

---

# 23. No Repository Trait by Default

Do not introduce repository traits merely because a repository module exists.

Avoid:

```rust
#[async_trait]
trait DictionaryRepository {
    async fn find(...);
}
```

plus:

```rust
struct PgDictionaryRepository;
```

unless there is a concrete need for interchangeable implementations.

The current backend does not require abstract repository interfaces simply to wrap SQLx.

Prefer ordinary private functions.

---

# 24. Transaction Ownership

Use cases own transaction boundaries.

Repositories execute persistence operations inside a transaction provided by the caller.

Example:

```rust
let mut tx = self.pool.begin().await?;

repository::find_parent(&mut tx, ...).await?;
repository::update_node(&mut tx, ...).await?;
repository::update_descendants(&mut tx, ...).await?;

tx.commit().await?;
```

Avoid repositories silently starting independent transactions for individual steps of one business operation.

The rule is:

> A transaction belongs to the business operation whose atomicity it protects.

Transaction semantics should remain visible in the use-case module.

---

# 25. Service / Use-Case Ownership

When a feature exposes a public capability facade, keep that facade in `service.rs`.

For a cohesive feature, `service.rs` may also own the use-case methods:

```text
business validation
authorization
workflow orchestration
transaction boundaries
audit coordination
repository calls
other capability calls
```

However, `service.rs` must not become the permanent catch-all file for every future method.

When use cases split into separate modules, keep the facade and construction in `service.rs`:

```text
service.rs
    public capability facade / construction

<business>.rs
    use-case implementation

repository.rs
    relational persistence
```

---

# 26. Complex Feature Growth

When a feature develops multiple independently nameable responsibilities, split by use case.

Do not split only because the file exceeds an arbitrary line count.

A complex feature may move from:

```text
feature/
└── service.rs
```

to:

```text
feature/
├── service.rs
├── repository.rs
├── capability_a.rs
└── capability_b.rs
```

Keep the public facade type and constructor in `service.rs`.

Multiple modules may implement the same facade type.

Example:

```rust
impl Accounts {
    // administrative workflows
}
```

in one file, and:

```rust
impl Accounts {
    // identity workflows
}
```

in another.

Do not create a new service type merely because the implementation moved to a different file.

Do not move the facade struct into `mod.rs` merely because use cases left `service.rs`.

---

# 27. When to Split a Module

Split a module when one or more of these are true:

* it contains multiple independently nameable responsibilities;
* different workflows have different dependencies;
* unrelated code changes frequently touch the same file;
* business invariants clearly differ between sections;
* developers routinely search through unrelated logic;
* tests naturally group around different capabilities.

Do not split only because:

```text
the file is over N lines
another feature has more files
a template says repository.rs should exist
```

File size is evidence, not the architectural reason.

---

# 28. Metadata Target Structure

## Dictionaries

Dictionaries currently contain two distinct responsibilities:

1. dictionary catalog management;
2. hierarchical dictionary detail/tree management.

Target:

```text
metadata/src/dictionaries/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
├── service.rs
├── catalog.rs
└── tree.rs
```

`service.rs` owns the public facade and construction:

```rust
DictionaryService
```

`catalog.rs` and `tree.rs` implement use cases on that facade.

The refactor must preserve existing public method names and signatures unless a separate API-change decision is made.

The first migration should split implementation modules, not capability identity.

---

## `service.rs`

Owns:

```text
DictionaryService struct
constructor
shared dependencies
```

Do not place catalog or tree workflow logic here once those use cases have been split out.

---

## `catalog.rs`

Owns use cases such as:

```text
list dictionaries
find dictionary
create dictionary
update dictionary
delete dictionary
import
export
```

It may coordinate repository calls and catalog-specific business rules.

---

## `tree.rs`

Owns:

```text
create detail
update/move detail
delete detail
find detail
tree lookup
children lookup
path lookup
parent validation
level/path recalculation
descendant updates
```

Tree transactions belong here.

---

## `repository.rs`

Owns PostgreSQL operations shared by catalog and tree workflows.

It must not absorb tree invariants merely because those invariants require SQL.

For example:

```text
parent cannot be the node itself
parent cannot be a descendant
descendant paths must be recalculated
```

remain tree business semantics.

---

## Parameters

Parameters remain a simple feature.

Target:

```text
metadata/src/parameters/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
└── service.rs
```

Do not split parameters further unless a new responsibility actually emerges.

If extracting `repository.rs` produces only trivial indirection with no improved responsibility boundary, the migration may be deferred.

---

# 29. IAM Target Structure

The top-level IAM capability boundaries remain:

```text
iam/src/
├── access/
├── accounts/
├── authorization/
├── departments/
├── menus/
└── roles/
```

These boundaries should not be replaced.

---

# 30. Accounts Target Structure

Accounts already contains several distinct use-case groups.

Target:

```text
accounts/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
├── service.rs
├── administration.rs
├── identity.rs
├── profile.rs
└── access.rs
```

`service.rs` owns the public facade and construction:

```rust
pub struct Accounts {
    ...
}
```

Do not replace it with multiple new public service types, and do not move the facade into `mod.rs` merely because use cases were split out.

---

## `administration.rs`

Owns workflows such as:

```text
list users
create user
update user
delete user
administrative visibility
department boundary
```

---

## `identity.rs`

Owns:

```text
login_account
refresh_identity
password_hash
update_password
```

---

## `profile.rs`

Owns:

```text
current user information
update current user
update current-user settings
```

---

## `access.rs`

Owns account access views and account-specific role/permission projection workflows.

The IAM authorization semantics themselves remain governed by `iam.md`.

---

# 31. Accounts Migration Constraint

When accounts is reorganized:

* preserve `Accounts` as the public facade;
* preserve public method signatures;
* move model/request types out of `mod.rs` in the same migration when doing so avoids immediately moving them again in a second PR;
* do not change IAM semantics as part of the directory refactor.

---

# 32. Roles

Roles may initially follow the simple feature pattern:

```text
roles/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
└── service.rs
```

Do not split further unless distinct responsibilities emerge.

Role Access semantics remain governed by `iam.md`.

---

# 33. Departments

Departments may initially use:

```text
departments/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
└── service.rs
```

Hierarchy rules may remain in `service.rs` while the feature is cohesive.

If department hierarchy operations later become a distinct sub-capability, introduce:

```text
tree.rs
```

at that time.

---

# 34. Menus

Menus may use:

```text
menus/
├── mod.rs
├── error.rs
├── model.rs
├── repository.rs
└── service.rs
```

Do not mechanically introduce additional files.

---

# 35. Authorization Is an Explicit Exception

Authorization already has a meaningful internal shape:

```text
authorization/
├── mod.rs
├── error.rs
├── service.rs
├── engine.rs
└── store.rs
```

Responsibilities remain:

```text
Authorization
    facade/coordinator

EnforcementEngine
    process-local last-good snapshot
    authorization evaluation
    Casbin Management mutations
    reload/watcher

PolicyStore
    authoritative DB reads for policy facts
```

Do not rename or reorganize these modules merely to match the generic repository pattern.

`PolicyStore` is already a real persistence boundary.

IAM policy write, reload, and best-effort audit semantics remain defined by `iam.md`.

In particular:

> Audit must not be moved into a repository or made transactionally atomic with IAM policy solely because modules are reorganized.

---

# 36. Auth Target Structure

Authentication currently has separate captcha/JWT/password concerns and a large token/session implementation.

Target:

```text
auth/src/
├── lib.rs
├── captcha.rs
├── jwt.rs
├── password.rs
└── token/
    ├── mod.rs
    ├── session.rs
    ├── refresh.rs
    └── store.rs
```

The public `TokenService` API should remain stable during the structural migration.

---

## `token/mod.rs`

Owns:

```text
TokenService
TokenPair
shared public token types
module exports
```

---

## `session.rs`

Owns session use cases:

```text
create login session
decode/check active session
revoke session
revoke all sessions for a user
```

---

## `refresh.rs`

Owns refresh-token semantics:

```text
RefreshGrant
inspect refresh token
rotate refresh token
refresh-token parsing
refresh-secret hashing
```

---

## `store.rs`

Owns Redis persistence details:

```text
Redis keys
session hashes
user-session indexes
Lua scripts
session delete/update operations
```

Business token/session semantics remain in `session.rs` and `refresh.rs`.

Do not introduce a Redis repository trait without a real alternate implementation requirement.

---

# 37. Repository vs Store vs Backend

These terms intentionally mean different things.

## `repository`

Use for relational/domain persistence.

Typical backing system:

```text
PostgreSQL
```

Examples:

```text
accounts/repository.rs
dictionaries/repository.rs
parameters/repository.rs
```

---

## `store`

Use when the abstraction represents state/session/policy persistence rather than normal domain-record CRUD.

Examples:

```text
authorization/store.rs
token/store.rs
```

Typical backing systems:

```text
Redis
policy facts
session state
```

---

## `backend`

Use for external I/O implementations.

Example:

```text
storages/backend.rs
```

Typical responsibilities:

```text
local filesystem
S3
OpenDAL operator creation
external storage configuration
```

Do not rename all persistence concepts to `repository` for superficial consistency.

---

# 38. File Storage Is an Explicit Exception

File storage combines:

```text
database metadata
object I/O
upload lifecycle
recovery
locking
transactional coordination
```

Its current responsibility-oriented structure is valid.

For example:

```text
files/
├── catalog.rs
├── objects.rs
├── service.rs
└── upload/
```

Do not force a generic `repository.rs` into file-storage merely because other SQL-backed CRUD features have one.

Refactor file-storage only when a clear responsibility boundary emerges.

---

# 39. Audit

Audit already contains distinct concepts such as:

```text
event
analysis
service
error
```

No directory rewrite is required solely for consistency.

A repository may be introduced later only if persistence becomes meaningfully independent from audit orchestration.

Do not split audit merely to match another crate's directory shape.

---

# 40. Public Capability API

A public capability API must be intentionally usable.

If a public method accepts a type:

```rust
pub async fn list(&self, query: SomeQuery)
```

external callers should be able to construct `SomeQuery`.

Every exposed type should be deliberately one of:

```text
public and usable
```

or:

```text
internal and hidden
```

Avoid accidentally public implementation details.

Persistence modules remain internal.

---

# 41. Capability Facades

Structural refactors should preserve capability identity.

Examples:

```text
DictionaryService
Accounts
RoleService
ParameterService
TokenService
```

should remain the public entry points unless a separate architecture decision changes the capability API.

The rule is:

> Split implementation modules, not capability identity.

When a public facade exists, keep it in `service.rs`.

Use-case modules may implement methods on that facade, but they do not replace it.

Do not turn one facade into multiple public service types merely because implementation methods move to separate files.

---

# 42. Naming Rules

Method names describe semantics rather than transport mechanics.

Preferred vocabulary:

```text
is_* / has_*
    boolean checks

require_*
    precondition guard
    no business mutation

validate_*
    input/invariant validation

ensure_*
    make a state true
    may mutate

create/update/delete/replace
    explicit mutations

load/get/find/list
    reads
```

Avoid transport-derived names such as:

```text
*_by_id
*_by_query
get_*_list
```

unless the qualifier is a genuine domain lookup dimension.

Example:

```text
get_parameter_by_key
```

is meaningful because the key is part of the business lookup semantics.

---

# 43. Module Visibility

Use the narrowest visibility that matches real responsibility.

Preferred progression:

```text
private
pub(super)
pub(in crate::<module>)
pub(crate)
pub
```

Do not use `pub(crate)` by default for internal helpers merely because it is convenient.

Only public capability APIs should normally use `pub`.

Internal persistence and coordination helpers should remain as narrow as possible.

---

# 44. Testing Placement

Tests should stay close to the responsibility they protect.

Private module behavior may use:

```rust
#[cfg(test)]
mod tests
```

inside the relevant module.

Public capability behavior and infrastructure integration may use crate-level integration tests.

When splitting a module, move tests with the responsibility they exercise.

Do not centralize unrelated tests into one catch-all test file solely for consistency.

---

# 45. Migration Rules

Backend architecture changes must be incremental.

Do not rewrite the entire backend in one pull request.

Every migration PR should preserve behavior unless behavior change is explicitly the goal.

---

# 46. Migration Sequence

Validate architectural patterns in this order:

```text
architecture contract
  → complex SQL capability (dictionaries)
  → API error boundary
  → Redis/store capability (auth token)
  → complex IAM capability (accounts)
  → selective remaining IAM adoption
```

PR sequencing and progress for a concrete migration belong in local working notes, not in this contract.

Parameters repository extraction is optional and is not part of the required sequence.

---

# 47. Pull Request Constraints

Architecture migrations should follow these rules:

1. One architectural concern per PR.
2. Preserve HTTP paths during structural migrations.
3. Preserve stable HTTP error contracts.
4. Preserve public capability APIs unless an API change is explicitly approved.
5. Do not combine broad rename, directory rewrite, and behavior change.
6. Keep transaction semantics unchanged.
7. Keep tests green after every migration step.
8. Prefer moving an existing responsibility before inventing a new abstraction.
9. Do not add repository traits without a concrete requirement.
10. Do not duplicate DTOs for layer purity.
11. Do not restructure API and capability implementations simultaneously unless required.
12. Report the exact verification commands run.

---

# 48. Architecture Decision Checklist

When adding or moving code, ask:

```text
Is this HTTP-specific?
    → api

Is this a multi-capability HTTP-only workflow?
    → api/routes/<feature>

Is this reusable business policy?
    → owning capability

Is this PostgreSQL persistence?
    → repository

Is this state/session/policy persistence?
    → store

Is this external I/O implementation?
    → backend

Is this capability input?
    → request

Is this domain/read data?
    → model

Is this a domain failure?
    → capability error

Is this how a failure appears to HTTP clients?
    → api/error
```

If ownership is still unclear after answering these questions, do not create a new abstraction immediately. Re-evaluate whether the responsibility is actually distinct.

---

# 49. Final Principles

## Capability-first

Prefer:

```text
iam/accounts
metadata/dictionaries
file-storage/files
```

over global technical-layer directories.

---

## Explicit dependency direction

Prefer:

```text
route
  ↓
use case
  ↓
repository / store / backend
```

Do not let lower layers call transport code.

---

## Business logic stays above persistence

Persistence modules know how data is stored.

Use-case modules know why and when persistence occurs.

---

## Transactions are business semantics

The use case owns the atomic boundary.

---

## Domain errors stay domain errors

HTTP representation belongs to `crates/api`.

---

## Multi-capability workflows may remain at the API edge

Do not invent a new architecture layer solely to move login or refresh sequencing elsewhere.

---

## Avoid ceremonial abstraction

Do not add:

```text
repository traits
duplicate DTOs
extra service types
empty architecture directories
generic adapter interfaces
```

without a real need.

---

## Preserve meaningful exceptions

`authorization/store.rs`, `file-storage` lifecycle modules, and storage `backend.rs` do not need to look like CRUD repositories.

Architecture consistency means consistent responsibility semantics, not identical directory trees.

---

## Split by responsibility, not by size

A large file is a signal.

A distinct responsibility is the reason to split.

---

## Keep public facades stable

Refactor implementation modules without unnecessarily changing capability identity.

---

# 50. Desired End State

A developer navigating the backend should be able to determine code ownership from responsibility alone.

The intended mental model is:

```text
Transport
    HTTP concerns

Capability
    business rules and workflows

Repository / Store / Backend
    persistence and external I/O

API Error Boundary
    domain failure → HTTP contract
```

If that ownership is obvious, the architecture is working.

> Make the obvious thing obvious.
