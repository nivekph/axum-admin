# Backend Architecture Refactoring Execution Plan

This plan describes how to migrate the current backend toward the architecture defined in:

```text
docs/architecture/backend-modules.md
```

It is an execution plan for this migration, not a second architecture contract.

```text
backend-modules.md
    durable responsibility rules

backend-modules-migration.md
    how this migration lands, PR by PR
```

The migration is intentionally incremental.

The objective is not to make the repository immediately match a target directory tree. Each phase should validate one architectural boundary before that pattern is applied elsewhere.

The migration must preserve existing behavior, HTTP contracts, and public capability APIs unless a separate change explicitly states otherwise.

---

# Status

| PR | Pattern | Status |
| --- | ------- | ------ |
| #75 | Architecture contract + execution plan | In this PR |
| #76 | Dictionaries repository + use-case split | Pending |
| #77 | API error ownership | Pending |
| #78 | Auth token store pattern | Pending |
| #79 | Accounts use-case split | Pending |
| #80 | Remaining IAM persistence alignment | Pending |
| optional | Parameters repository extraction | Deferred; skip unless justified after #76 |

Update this table when a PR lands. Do not reserve later GitHub issue/PR numbers until earlier phases are complete.

The recommended sequence is **6 main PRs + 1 optional PR**. Do not fragment further into one-PR-per-directory for later selective adoption.

---

# 1. Execution Principles

Every refactoring PR should follow these rules:

1. One architectural concern per PR.
2. No unrelated behavior changes.
3. Preserve public capability APIs.
4. Preserve HTTP routes, status codes, error codes, and response shapes.
5. Keep transaction semantics unchanged.
6. Do not restructure capability and API route modules in the same PR.
7. Introduce abstractions only when the responsibility already exists.
8. Use the narrowest module visibility possible.
9. Move tests with the responsibility they verify.
10. Run the narrowest relevant checks first, then the workspace checks.

The migration strategy is:

```text
document
   ↓
validate complex SQL capability (dictionaries)
   ↓
clean HTTP error boundary
   ↓
validate Redis/store capability (auth token)
   ↓
validate complex IAM capability (accounts)
   ↓
apply proven patterns selectively (remaining IAM)
```

Parameters repository extraction is optional and is not part of the required sequence.

---

# 2. Phase 1 — Freeze the Architecture Contract

## Goal

Commit the backend module architecture before changing more implementation structure.

## Changes

Add:

```text
docs/architecture/backend-modules.md
```

Update:

```text
AGENTS.md
```

to reference the architecture document.

Do not duplicate the full architecture rules inside `AGENTS.md`.

Add a concise instruction such as:

```text
Read docs/architecture/backend-modules.md before changing backend
module boundaries, persistence layout, transaction ownership,
or API error organization.
```

Update stale references that describe:

```text
crates/api/src/mappings.rs
```

as the permanent API error architecture.

Existing domain-specific documents remain authoritative:

```text
docs/architecture/api-dto-ownership.md
docs/architecture/iam.md
```

## Out of Scope

* Rust code movement
* handler changes
* SQL changes
* behavior changes
* public API changes

## Acceptance Criteria

* architecture responsibilities are documented;
* `AGENTS.md` points to the new architecture contract;
* DTO and IAM documents remain the canonical source for their specific rules;
* no runtime behavior changes.

## Suggested PR

```text
docs: define backend module architecture
```

---

# 3. Phase 2 — Refactor Metadata Dictionaries

This is the first real architecture validation.

## Why Dictionaries First

The current dictionaries implementation mixes:

```text
dictionary catalog CRUD/import/export
+
dictionary detail tree operations
+
tree invariants
+
transactions
+
SQL
```

in one service module.

It therefore validates several architecture rules at once:

```text
repository boundary
use-case ownership
transaction ownership
complex feature splitting
facade stability
module visibility
```

## Target Structure

```text
crates/metadata/src/dictionaries/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
├── catalog.rs
└── tree.rs
```

Remove:

```text
service.rs
```

once its responsibilities have been moved.

---

## `repository.rs`

Move persistence-only operations here.

Examples:

```text
find dictionary
find dictionary by type
insert dictionary
update dictionary
delete dictionary records
find detail
insert detail
update detail row
delete detail
load children
load descendants
```

Rules:

* SQL belongs here;
* row mapping may live here when persistence-specific;
* no HTTP concepts;
* no authorization;
* no audit orchestration;
* no business workflow decisions;
* no public repository API.

Default visibility:

```rust
pub(super)
```

where possible.

---

## `catalog.rs`

Own:

```text
list
find
create
update
delete
import
export
```

Keep the existing facade:

```rust
DictionaryService
```

Example:

```rust
impl DictionaryService {
    pub async fn list(...) { ... }
    pub async fn create(...) { ... }
}
```

Do not introduce:

```text
DictionaryCatalogService
```

---

## `tree.rs`

Own:

```text
create_detail
update_detail
find_detail
delete_detail
tree_by_dictionary
tree_by_type
details_by_parent
detail_path
```

Also own tree semantics:

```text
parent validation
cycle prevention
level calculation
path calculation
descendant recalculation
```

Transaction boundaries stay here.

Example:

```rust
let mut tx = self.pool.begin().await?;

repository::load_parent(&mut tx, ...).await?;
repository::update_detail(&mut tx, ...).await?;
repository::update_descendants(&mut tx, ...).await?;

tx.commit().await?;
```

Repository functions must not independently commit pieces of the same tree operation.

---

## Public API Constraint

The following must remain stable:

```text
DictionaryService
existing public methods
existing public inputs
existing public outputs
```

This is an internal structural refactor.

---

## API Constraint

Do not change:

```text
crates/api/src/routes/dictionaries/
```

in this phase.

In particular, do not simultaneously split API handlers into:

```text
catalog.rs
tree.rs
```

That decision is deferred until the capability structure has been validated.

---

## Verification

Run at minimum:

```bash
cargo test -p metadata
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

If the workspace already has a different CI command convention, use the repository-standard equivalent.

## Suggested PR

```text
refactor(metadata): split dictionary persistence and tree workflows
```

---

# 4. Phase 3 — Refactor the API Error Boundary

## Goal

Remove the catch-all API error mapping file and make HTTP error ownership explicit.

## Current Direction

Replace:

```text
crates/api/src/error.rs
crates/api/src/mappings.rs
```

with:

```text
crates/api/src/error/
├── mod.rs
├── transport.rs
├── auth.rs
├── iam.rs
├── metadata.rs
├── storage.rs
└── audit.rs
```

Keep:

```text
crates/api/src/response.rs
```

separate.

---

## `error/mod.rs`

Own:

```text
AppError
AppResult
ErrorKind
ErrorSpec
IntoResponse
generic JSON rejection handling
generic internal error handling
public error sanitization
```

---

## `error/transport.rs`

Own transport/middleware-specific contracts such as:

```text
RATE_LIMITED
RATE_LIMIT_UNAVAILABLE
```

Do not create:

```text
misc.rs
```

as a general bucket.

---

## Capability Error Modules

Use ownership by source capability:

```text
auth.rs
iam.rs
metadata.rs
storage.rs
audit.rs
```

Examples:

```text
AccountError -> error/iam.rs
DictionaryError -> error/metadata.rs
FileError -> error/storage.rs
AuditAnalysisError -> error/audit.rs
```

AI analysis errors remain under:

```text
error/audit.rs
```

because Audit owns that capability.

---

## Context-Specific Errors

Do not force route-specific semantics into global `From` implementations.

For example, login may intentionally collapse:

```text
unknown user
wrong password
```

into:

```text
INVALID_CREDENTIALS
```

through route-local `LoginError`.

A shared `ErrorSpec` does not imply that a global:

```rust
impl From<...> for AppError
```

must exist.

---

## Contract Constraint

Preserve:

```text
HTTP status
error code
public message
response envelope
```

exactly.

This PR is organizational only.

---

## Documentation Updates

Update:

```text
AGENTS.md
docs/architecture/iam.md
```

where they still reference:

```text
crates/api/src/mappings.rs
```

as the error mapping location.

Do not redefine the error rules in those documents; update references only.

---

## Verification

```bash
cargo test -p api
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Pay particular attention to tests asserting stable public error codes.

## Suggested PR

```text
refactor(api): organize HTTP errors by ownership
```

---

# 5. Optional — Evaluate Metadata Parameters

This is **not** part of the required 6-PR sequence.

## Goal

Optionally validate the simple SQL-backed feature pattern after dictionaries (#76).

Expected target if proceeding:

```text
crates/metadata/src/parameters/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
└── service.rs
```

## Important Constraint

Do not extract `repository.rs` merely because the architecture document contains that filename.

First evaluate whether separating SQL from `service.rs` creates a meaningful responsibility boundary.

Proceed only when the result is clearer than:

```text
service.rs
    calls one trivial repository function per method
```

If repository extraction creates only ceremonial delegation, leave the feature unchanged and record:

```text
No change required; current module is sufficiently cohesive.
```

That is a valid architectural result.

## If Proceeding

`repository.rs` owns SQL.

`service.rs` owns capability semantics and remains the public facade.

Preserve:

```text
ParameterService
public inputs
public outputs
```

## Suggested PR

If useful:

```text
refactor(metadata): isolate parameter persistence
```

Insert it after #76 only when justified. Do not block #77–#80 on this decision.

---

# 6. Phase 5 — Refactor Auth Token Lifecycle

## Goal

Validate the `store` pattern for state/session persistence.

## Target Structure

```text
crates/auth/src/token/
├── mod.rs
├── session.rs
├── refresh.rs
└── store.rs
```

Replace the current single:

```text
token.rs
```

---

## `token/mod.rs`

Own:

```text
TokenService
TokenPair
shared public token types
shared error exports where appropriate
```

The public facade remains:

```text
TokenService
```

---

## `session.rs`

Own:

```text
create_session
decode_active
revoke
revoke_user_sessions
```

These are session lifecycle semantics.

---

## `refresh.rs`

Own:

```text
RefreshGrant
inspect_refresh
rotate_refresh
refresh token parsing
refresh token hashing
```

---

## `store.rs`

Own Redis persistence:

```text
session keys
user-session keys
HGET/HSET
ZADD/ZREM
Redis scripts
session storage operations
```

Do not move refresh/session business semantics into `store.rs`.

Do not add:

```text
TokenStore trait
RedisTokenRepository
```

unless a real alternate implementation requirement exists.

---

## Public API Constraint

Preserve:

```text
TokenService
TokenPair
existing public methods
existing public error semantics
```

## Verification

Run token unit/integration tests including Redis-backed tests, then:

```bash
cargo test -p auth
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Suggested PR

```text
refactor(auth): split token session, refresh, and store responsibilities
```

---

# 7. Phase 6 — Refactor IAM Accounts

## Goal

Validate a large use-case-oriented capability while preserving one public facade.

## Target Structure

```text
crates/iam/src/accounts/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
├── administration.rs
├── identity.rs
├── profile.rs
└── access.rs
```

Remove the large catch-all:

```text
service.rs
```

after migration.

---

## `administration.rs`

Own:

```text
list users
create user
update user
delete user
visibility rules
department boundary
administrative scope
```

---

## `identity.rs`

Own:

```text
login_account
refresh_identity
password_hash
update_password
```

---

## `profile.rs`

Own:

```text
current user info
update current user
update user settings
```

---

## `access.rs`

Own:

```text
account access view
assigned role projection
effective permission projection
account-specific access workflows
```

IAM policy semantics remain defined by:

```text
docs/architecture/iam.md
```

---

## `repository.rs`

Own account PostgreSQL operations.

It must not own:

```text
department authorization
role authorization
policy mutations
audit policy
HTTP behavior
```

---

## Facade Constraint

Keep:

```rust
Accounts
```

as the single public capability facade.

Do not introduce:

```text
AccountAdminService
IdentityService
ProfileService
```

solely because code moved into separate modules.

---

## Model / Request Migration

Current model/request definitions that still live in `mod.rs` should be moved during this phase when doing so prevents another immediate structural PR.

The goal is to avoid:

```text
PR 1: split service
PR 2: move models
PR 3: move request types
```

when those moves are part of the same obvious module-boundary cleanup.

---

## IAM Constraint

Do not change:

```text
authorization semantics
role semantics
permission semantics
audit ordering
reload semantics
super_admin behavior
```

during this refactor.

Policy mutation remains:

```text
validate
→ persist policy
→ notify reload
→ best-effort audit
```

as defined by `iam.md`.

---

## Verification

At minimum:

```bash
cargo test -p iam
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Also run IAM-specific integration tests covering:

```text
account visibility
create/update/delete
login identity lookup
password update
role/access views
authorization boundaries
```

## Suggested PR

```text
refactor(iam): split account workflows by responsibility
```

---

# 8. Phase — Remaining IAM Features (#80)

Review together in one PR:

```text
roles
departments
menus
```

Do not open one PR per directory.

Do not assume all three require changes.

For each feature, ask:

```text
Is SQL mixed with meaningful business orchestration?

Would repository extraction clarify ownership?

Are there multiple independently nameable use cases?

Would a split reduce coupling, or only add delegation?
```

Possible simple shape:

```text
feature/
├── mod.rs
├── error.rs
├── model.rs
├── request.rs
├── repository.rs
└── service.rs
```

But this is not mandatory.

Each feature receives its own decision inside the same PR.

Possible outcomes:

```text
refactor now
defer
no change required
```

All three are acceptable.

Also confirm in the same review that intentional exceptions remain unchanged unless new evidence appeared:

```text
authorization/service+engine+store
file-storage lifecycle modules
audit capability layout
api/routes mirroring of capability splits
```

The default for those exceptions remains:

```text
no change
```

## Suggested PR

```text
refactor(iam): align remaining IAM persistence boundaries
```

---

# 9. Explicit Exceptions Remain Deliberate

After the main sequence, do not revisit these without new evidence.

---

## Authorization

Do not revisit the existing:

```text
service.rs
engine.rs
store.rs
```

split without new evidence.

Its responsibilities are already documented and deliberate.

---

## File Storage

Do not introduce a generic repository simply to match Metadata/IAM.

Its lifecycle-oriented structure is an intentional exception.

---

## API Routes

Only consider splitting large API features after their owning capability structure has stabilized.

For example, dictionaries may eventually become:

```text
routes/dictionaries/
├── mod.rs
├── dto.rs
├── catalog.rs
└── tree.rs
```

but only if the route responsibilities themselves justify the split.

Do not mirror domain directories automatically.

---

# 10. PR Sequence

The recommended sequence is **6 main PRs + 1 optional PR**:

| PR | Title | Primary architecture pattern |
| --- | ----- | ---------------------------- |
| #75 | Backend architecture contract | Freeze responsibilities + execution plan |
| #76 | Metadata dictionaries | Repository + complex use-case split |
| #77 | API error boundary | HTTP error ownership |
| #78 | Auth token | Store/state pattern |
| #79 | IAM accounts | Large facade + multiple use-case modules |
| #80 | Remaining IAM | Selective persistence alignment for roles/departments/menus |

Optional:

| PR | Title | When |
| --- | ----- | ---- |
| — | Metadata parameters repository | Only if #76 shows the extraction is more than trivial forwarding |

Suggested PR titles:

```text
docs: define backend module architecture

refactor(metadata): split dictionary persistence and tree workflows

refactor(api): organize HTTP errors by ownership

refactor(auth): split token session, refresh, and store responsibilities

refactor(iam): split account workflows by responsibility

refactor(iam): align remaining IAM persistence boundaries
```

Do not split #80 into one PR per feature directory. Review `roles`, `departments`, and `menus` together; change only what the already-validated patterns clearly justify.

Later GitHub numbers should not be treated as reserved until earlier PRs exist.

---

# 11. Review Checklist for Every Refactor PR

Before approving a structural backend PR, verify:

## Responsibility

* Does every moved module have a clear responsibility?
* Was anything split only because the file was large?
* Was any new abstraction added only for symmetry?

## Dependency Direction

* Does API still depend on capabilities rather than the reverse?
* Does persistence stay below use cases?
* Did a repository gain authorization or HTTP knowledge?

## Transactions

* Is the transaction boundary still owned by the business operation?
* Did repository extraction accidentally change atomicity?
* Did previously one-transaction logic become multiple transactions?

## Public API

* Are public facade types unchanged?
* Are method signatures unchanged?
* Are externally constructible public request types still usable?

## HTTP Contract

For API changes:

* same path;
* same status;
* same error code;
* same public message;
* same response envelope.

## Visibility

* Are persistence modules private?
* Are helpers using the narrowest practical visibility?
* Was `pub(crate)` added unnecessarily?

## Testing

* Did tests move with the responsibility they cover?
* Were narrow tests run first?
* Did the workspace tests pass?
* Did Clippy and formatting pass?

---

# 12. Stop Conditions

Do not continue mechanically through the plan.

Pause a migration pattern when:

```text
repository extraction adds only delegation
a proposed split cannot be named by responsibility
public API changes become necessary
transaction semantics become less visible
two modules become tightly coupled through many private helpers
a supposedly generic pattern conflicts with an existing deliberate exception
```

In those cases, prefer retaining the current structure over forcing the target tree.

The architecture document defines responsibility rules, not a migration quota.

---

# 13. Completion Criteria

This migration is complete when:

* the architecture contract is committed;
* dictionaries demonstrate a clear repository/use-case split;
* API error ownership no longer depends on a catch-all mapping module;
* token/session persistence is separated from token semantics;
* accounts no longer rely on one catch-all service implementation;
* remaining features have been deliberately reviewed rather than mechanically rewritten;
* authorization and file-storage remain unchanged unless new evidence justified changes;
* public capability facades remain stable;
* no global technical-layer directory structure has been introduced.

The desired outcome is not that every directory looks identical.

The desired outcome is that a developer can answer:

```text
Where does this rule belong?
Where does this SQL belong?
Who owns this transaction?
Who exposes this capability?
Who decides how this error looks over HTTP?
```

without guessing.
