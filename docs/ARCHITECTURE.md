# Architecture

## Scope

ZAPITRA is a finite reference model for the public BLAKE3-Servil Pipeline design. The model uses Rust standard-library data structures. It does not call the upstream skeleton as a library because its methods contain `todo!()` bodies.

## State

A model state contains:

- A bounded map of open or in-flight message records (`limits.messages`).
- A separate bounded queue of finished events (`limits.messages`).
- A fixed vector of room slots (`limits.rooms`).
- Caller-held Lent guard state.
- A monotonically increasing internal message ID and completion sequence.

Caller tags are payload values. The model uses small `u8` tags for finite exploration and permits duplicates. Internal message IDs distinguish messages that use the same tag.

## Message and room lifecycle

A message starts without an input path. It may use one path only: committed rooms, `owned`, `lent`, or `batch`. A room is acquired without a message association. `commit` assigns the room to a message and records the in-flight room count. A room returns to the pool when its abstract worker completes the room.

A message with committed rooms becomes terminal through `end`, `abort`, or a dropped Message observed at the next Pipeline call. Its event is produced only after its committed room work has drained.

## Completion-event pressure

The design describes separate capacities for message records and finished events. It does not fully describe the representation of a completed result when the event queue is full. ZAPITRA retains that result in the originating message record until `next_event` opens a slot. The report and README call this an assumption. It is an explicit question for the maintainers.

The model preserves completion order. A pending result is promoted by the smallest completion sequence when an event slot opens.

## Wait classification

Three waits are modeled:

- `message().await`: an open message slot is available, a worker-only schedule can free one, the caller must end/abort an open message, or the consumer must pull from a full event queue.
- `room().await`: a room is available, a worker-only schedule can finish a committed room, or the caller holds all rooms uncommitted.
- `ready().await`: an event is already queued, a worker-only schedule can produce one, or no worker action can produce one without caller action.

The worker-only oracle performs breadth-first reachability over worker transitions. It shares the reference state's transition functions with the main explorer. It is an independent search policy, not an independent implementation of the upstream library.

## Lent abstraction

A Lent guard is modeled separately from the completion event. Two abstract worker units expose partial-start cancellation: dropping the guard cancels units not started and waits for already-running units. The value two is a scheduler abstraction, not a claim about BLAKE3's internal work partitioning.

ZAPITRA selects the limited-Lent policy (option 2). The guard protects the borrowed bytes while abstract readers are active. Dropping it before the terminal event is materialized cancels unstarted work and drains active readers; the message then produces `Aborted`. Once all readers finish and a `Hashed` event is materialized, the guard may be released even if the bounded event pool delays the caller's `next_event()` call. The event-creation boundary is an explicit interpretation of “before the message's event” and is presented to maintainers for confirmation.

## Token provenance

`MessageToken` and `RoomToken` carry a model Pipeline ID. Operations reject tokens from another model instance. The explorer's compact state transitions use internal IDs; dedicated token-operation tests exercise cross-instance rejection.

## Search algorithm

The explorer uses breadth-first search, a `BTreeSet` of visited states, and a queue of witness paths. Each accepted transition is checked against invariants. For every blocked wait, the model records a classification and compares it with worker-only reachability.

The search is bounded by depth and state count. A report indicates whether either bound truncated the traversal. A bounded pass is not a proof over all possible limits or schedules.

## Invariants

- Message-record count does not exceed `limits.messages`.
- Event count does not exceed its separate `limits.messages` capacity.
- Room vector length remains `limits.rooms`.
- Every in-flight room points to a live room-path message.
- Per-message in-flight room counts match room assignments.
- Each message uses at most one byte-input path.
- Direct-input tokens exist only on the matching input path.
- Finished-pending records have no remaining worker work and retain completion order.
- No message has two terminal events.
- Lent guard state matches the message/event lifecycle.
- Message and room tokens carry their originating model Pipeline identity.

## Deliberate exclusions

The current model does not execute BLAKE3, inspect actual digest bytes, size buffers from `length_hint`, simulate Pipeline destruction, model real thread scheduling, or interact with kernel I/O. Those need an implementation branch and a separate runtime adapter.
