# Contract matrix

| Design area | Model representation | Evidence produced | Scope caveat |
|---|---|---|---|
| Message capacity | Bounded message-record map | Invariant checks and wait witnesses | Finite positive limits |
| Event capacity | Separate bounded event queue | Event-full scenarios | Full-queue result retention is a model assumption |
| Room capacity | Fixed room vector | Room pool invariants | Abstract worker completion |
| Room acquisition | Unassigned `Held` room | Acquire then commit to any open message | No actual socket or kernel read |
| Input path | `Rooms`, `Owned`, `Lent`, `Batch` | Mixed-path rejection | No actual hashing |
| Duplicate tags | `u8` tag plus internal ID | Duplicate-tag test | Tag value domain is bounded |
| Completion order | Monotonic finish sequence | Reordered worker-completion test | Abstract worker scheduler |
| Message drop | `drop_pending` observed on next Pipeline call | Aborted event test | No real Rust RAII integration with upstream |
| Lent cancellation | Active, event-materialized, event-delivered guard states | Early-cancel, post-materialization release, and full-queue deferred-event tests | Two abstract work units; selected event-materialization boundary is submitted for maintainer confirmation |
| Batch return | Paired batch/digest storage tokens | Batch event assertion | No actual `Batch` buffer is hashed |
| Message wait | Worker-only reachability plus event queue check | Wait-class audit and witness | Same reference transition relation |
| Room wait | Worker-only room reachability | Wait-class audit and witness | Same reference transition relation |
| Ready wait | Worker-only event reachability | Wait-class audit and witness | Does not block a real executor |
| Token provenance | Opaque model token includes Pipeline ID | Cross-instance operation tests | This is the reference model's wrapper |
| Compile-time API examples | Upstream doctest script | Upstream revision SHA and log | Skeleton has `todo!()` bodies |

## Result interpretation

A completed exploration means all reachable states within the selected depth and state budget were checked. `state_limit_hit` means the state budget stopped the search. `depth_limit_hit` means the explorer found enabled actions beyond the configured depth. Neither result establishes runtime conformance.
