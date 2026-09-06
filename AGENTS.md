# axum-admin

This file gives repo-specific guidance for agents working in this project.

## Project Shape

- Backend process entry points live in `apps/ava`; the Axum HTTP capability crates live under `crates/`.
- The React Admin Console lives in `apps/desktop` and runs as a browser SPA.
- The previous Vue application and its Tauri wrapper are preserved only in the `v1.1.0` tag.
- Database migrations live in `migrations/`.
- Uploaded local files are served from `uploads/`; do not commit generated upload data.

## Backend

- Read [`docs/architecture/backend-modules.md`](docs/architecture/backend-modules.md) before
  changing backend module boundaries, persistence layout, transaction ownership, or API error
  organization.
- Read [`docs/architecture/api-dto-ownership.md`](docs/architecture/api-dto-ownership.md) before
  changing API request or response DTOs, OpenAPI schemas, or capability-side `utoipa` derives.
- Use REST-style routes under `/api`.
- Public routes are registered in `crates/api/src/routes/public`.
- Authenticated routes are registered in `crates/api/src/routes/protected` and use the `Authorization: Bearer <token>` header.
- Keep response bodies in the shared envelope shape:

```json
{
  "code": "OK",
  "message": "ok",
  "data": {}
}
```

- Use `api::AppError` and the API error boundary for stable error codes and messages. The catch-all
  lives in `crates/api/src/mappings.rs` today; the target layout is `crates/api/src/error/` as defined
  in [`docs/architecture/backend-modules.md`](docs/architecture/backend-modules.md).
- Keep business logic in the owning capability crate (`crates/iam`, `crates/audit`, `crates/metadata`, `crates/file-storage`, etc.) rather than pushing it into route handlers.
- When adding SQL schema changes, create a new migration in `migrations/`; do not edit an already-applied migration unless the user explicitly confirms the database can be reset.
- Keep `sqlx::migrate!("../../migrations")` working from `crates/db`.
- Prefer explicit domain errors over generic string errors.

### IAM

- Read [`docs/architecture/iam.md`](docs/architecture/iam.md) before changing IAM, protected API
  routes, the access catalog, authentication middleware, or Admin Console access workflows.
- Keep Request Access, Accounts, Roles, Menus, and private Authorization responsibilities distinct.
  Router-level Authentication performs token/session work and checks User status once. A
  route-local Permission guard performs one Authorization decision for each protected management
  method; Self-Service routes have no Permission guard. HTTP handlers call Accounts or Roles for
  access administration, never extract the private guard context, and never call private
  Authorization directly.
- HTTP topology is owned by Axum route registration, not by the Access Catalog. Management methods
  attach one concrete Permission to their `MethodRouter` with `.permission(code)`; the Access
  Catalog does not store or expose HTTP method/path bindings, and Router construction must not
  validate against them.
- PostgreSQL is authoritative. Casbin's `SqlxAdapter` exclusively persists concrete Role Permission
  policy (`p`) and User-to-Role membership (`g`) through Casbin Management APIs. Do not query or
  mutate `casbin_rule` directly in application code. Authentication and route-local Permission
  evaluation read the process-local last-good Authorization snapshot and must not query PostgreSQL.
  Redis only propagates reload notifications.
- Access is role-only, additive, and allow-only. A User may have zero, one, or multiple Roles.
  Effective Permissions are the union of concrete Permissions from enabled assigned Roles. Do not
  add Direct Permissions, deny rules, wildcard grants, Role inheritance, configurable Data Scope,
  `is_system` authorization flags, or frontend role-code bypasses without an architecture change.
- Role Access is one directory/page/action tree. Directories are structural. Selecting an action
  includes its owning page Permission; selecting a page does not include its actions. Navigation is
  derived only from effective page Permissions and their directory ancestors.
- `super_admin` is an enabled, protected Role with concrete grants for every enabled Permission.
  Its code, status, access, and deletion are immutable through supported APIs; its memberships remain
  mutable, including removal of the final active membership. Recovery after removing all members is
  a manual database operation.
- Policy writes commit before best-effort audit recording. An audit failure is logged at high
  priority and does not roll back a successful policy mutation. Redis reload and periodic repair
  replace an Enforcer only after a complete successful load; a failed reload retains the last good
  Enforcer, so do not claim strict fail-closed policy freshness.

### Error Design

- Route and middleware handlers should return `api::AppResult<T>`. The route-local Permission
  middleware is the deliberate exception: it returns `Response` so both authorization errors and
  early denials remain inside Axum's `Infallible` layer boundary.
- `crates/api` owns the public HTTP boundary types `AppError`, `AppResult<T>`, and `ApiResponse<T>`.
- Repeated fixed HTTP contracts may use crate-private `ErrorSpec` constants. Consume them with ordinary `ok_or` and `?`; do not add per-error constructor helpers or extension traits.
- Keep stable error specs in the owning layer:
  - domain errors: the owning capability crate's local `error.rs` or `errors.rs`
  - API boundary errors: the API error boundary (currently `crates/api/src/mappings.rs`; target
    `crates/api/src/error/` in [`docs/architecture/backend-modules.md`](docs/architecture/backend-modules.md)),
    with route-local errors only for multi-capability workflows such as login
- Keep stable, context-independent conversions from private implementation errors into a domain error in the owning module's `error.rs`; service code should propagate them with `?`.
- Add `impl From<...> for AppError` only when the source error has one stable API meaning in every context.
- When the same error type has context-specific semantics, map it explicitly at the call site with `.map_err(...)`.
- Keep user-management and authentication errors distinct:
  - CRUD/user management returns `AccountError` from `crates/iam/src/accounts`.
  - Login uses the route-local `LoginError`; unknown users and incorrect passwords both become `INVALID_CREDENTIALS`.
  - Authentication calls `AccessService::require_active_user`; `AccessEvaluationError` maps a
    missing/deleted token user to `SESSION_INVALID` and a disabled user to `USER_DISABLED`.
    Route-local Permission guards call `AccessService::authorize_permission`; an ordinary denial
    maps to `PERMISSION_DENIED`.

## Frontend

- The Admin Console is React + Vite + React Router + Zustand + TanStack Query + TanStack Table + Axios + shadcn/ui on Base UI, with Tabler Icons.
- API wrappers live in `apps/desktop/src/api`; keep endpoint paths aligned with `crates/api/src/routes`.
- Keep the default API base URL as `http://127.0.0.1:3000/api` unless changing the runtime contract intentionally.
- Use the shared HTTP client in `apps/desktop/src/api/http.ts` so backend envelope errors surface through the same path.
- Keep UI changes consistent with the existing admin layout: dense, practical, and workflow-oriented.
- Add or update Vitest coverage when changing API wrappers, stores, router behavior, or view workflows.

## Rust Style

- Use the workspace dependencies declared in the root `Cargo.toml`.
- Keep local workspace crates listed before third-party dependencies.
- Prefer small modules with clear ownership over broad shared helpers.
- Avoid helper functions that are only used once unless they clarify a complex block.
- When using `format!`, inline variables in `{}` when possible.
- Prefer exhaustive `match` arms over wildcard arms when the enum is local and meaningful.
- Prefer resource-oriented handler names (`list_`, `get_`, `create_`, `update_`,
  `delete_`, `replace_`). Do not encode HTTP mechanics such as `by_id` or `by_query`
  in function names.
- Use verb prefixes by side effect:
  - `is_` / `has_`: boolean checks
  - `require_`: precondition guards that return an error and do not mutate
  - `validate_`: input or invariant checks that do not mutate
  - `ensure_`: make a state true, may insert or update
  - `create` / `update` / `delete` / `replace`: explicit mutations
  - `load` / `get` / `find` / `list`: reads
- Run formatting after Rust edits:

```bash
cargo fmt --all
```

## Verification

Use the narrowest meaningful check first, then broaden when shared behavior changed.

Backend:

```bash
cargo test --workspace
```

Frontend:

```bash
cd apps/desktop
pnpm test
pnpm build
```

For frontend/backend integration changes, run both servers and verify the real UI path:

```bash
cargo run -p ava serve
cd apps/desktop && pnpm dev
```

Bootstrap login:

```text
ADMIN_USERNAME / ADMIN_PASSWORD from the environment
```

Before claiming a change is complete, report the exact verification commands that were run and whether they passed.

## Agent skills

### Issue tracker

When the local `.notes/` tracker is present, use `.notes/agents/issue-tracker.md` for planning specs
and implementation issues. `.notes/` is local temporary working material and is not a source for
committed project documentation. When a design or decision needs to be retained or committed, move
the reviewed content into the appropriate location under `docs/` and update tracked references to
point there; do not make committed documentation depend on `.notes/`.

### Triage labels

When present, the local tracker uses the five-role vocabulary in `.notes/agents/triage-labels.md`.

### Domain docs

- Tracked, current architecture lives in `docs/architecture/`.
- Backend module organization is [`docs/architecture/backend-modules.md`](docs/architecture/backend-modules.md).
- IAM's implementation document is [`docs/architecture/iam.md`](docs/architecture/iam.md).
- Local `.notes/` files hold temporary planning context and task history and may describe superseded
  targets; use them for provenance, not as durable architecture or evidence that behavior is
  implemented. See `.notes/agents/domain.md` when the local tracker is available.
  The current backend module migration plan, when present, lives in
  `.notes/backend-modules-migration.md`.
