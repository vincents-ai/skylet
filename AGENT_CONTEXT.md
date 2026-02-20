# Restricted Environment Protocol

## ACCESS DENIED
- You do **not** have access to `cargo`, `rustc`, or `clippy`.
- Do not attempt to run them directly. It will fail.

## ALLOWED ACTIONS
| Action | Command |
| :--- | :--- |
| Check Syntax | `agent-check` |
| Build Release | `agent-build` |
| Add Dependency | `agent-add <crate>` (Auto-audits) |
| Auto-Fix Code | `agent-fix` |

