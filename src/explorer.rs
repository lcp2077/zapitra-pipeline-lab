//! Bounded breadth-first state-space exploration and wait-oracle audit.

use std::collections::{BTreeSet, VecDeque};
use std::fmt::Write as _;

use crate::model::{InputPath, Limits, RoomState, State, WaitClass};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Action {
    Open {
        tag: u8,
    },
    AcquireRoom,
    DropRoom {
        room: usize,
    },
    Commit {
        message: u16,
        room: usize,
    },
    End {
        message: u16,
    },
    Abort {
        message: u16,
    },
    DropMessage {
        message: u16,
    },
    Owned {
        message: u16,
        token: u16,
    },
    Lent {
        message: u16,
    },
    DropLentGuard {
        message: u16,
    },
    Batch {
        message: u16,
        batch: u16,
        digests: u16,
    },
    StartDirect {
        message: u16,
    },
    FinishDirect {
        message: u16,
    },
    FinishRoom {
        room: usize,
    },
    PullEvent,
}

impl Action {
    fn label(&self) -> String {
        match self {
            Self::Open { tag } => format!("message(tag={tag})"),
            Self::AcquireRoom => "room()".into(),
            Self::DropRoom { room } => format!("drop Room({room})"),
            Self::Commit { message, room } => {
                format!("commit(message={message}, room={room}, written=1)")
            }
            Self::End { message } => format!("end(message={message})"),
            Self::Abort { message } => format!("abort(message={message})"),
            Self::DropMessage { message } => format!("drop Message({message})"),
            Self::Owned { message, token } => format!("owned(message={message}, owner={token})"),
            Self::Lent { message } => format!("lent(message={message})"),
            Self::DropLentGuard { message } => format!("drop Lent guard({message})"),
            Self::Batch {
                message,
                batch,
                digests,
            } => {
                format!("batch(message={message}, batch={batch}, digests={digests})")
            }
            Self::StartDirect { message } => format!("worker_start_direct({message})"),
            Self::FinishDirect { message } => format!("worker_finish_direct({message})"),
            Self::FinishRoom { room } => format!("worker_finish_room({room})"),
            Self::PullEvent => "next_event() [pull]".into(),
        }
    }

    fn apply(&self, state: &mut State) -> Result<(), String> {
        match self {
            Self::Open { tag } => apply_blocking_call(state, "message wait:", |state| {
                state.open_message(*tag).map(|_| ())
            }),
            Self::AcquireRoom => apply_blocking_call(state, "room wait:", |state| {
                state.acquire_room().map(|_| ())
            }),
            Self::DropRoom { room } => state.drop_room(*room),
            Self::Commit { message, room } => state.commit(*message, *room, 1),
            Self::End { message } => state.end_message(*message),
            Self::Abort { message } => state.abort_message(*message),
            Self::DropMessage { message } => state.drop_message(*message),
            Self::Owned { message, token } => state.owned(*message, *token),
            Self::Lent { message } => state.lent(*message),
            Self::DropLentGuard { message } => state.drop_lent_guard(*message),
            Self::Batch {
                message,
                batch,
                digests,
            } => state.batch(*message, *batch, *digests),
            Self::StartDirect { message } => state.worker_start_direct(*message),
            Self::FinishDirect { message } => state.worker_finish_direct(*message),
            Self::FinishRoom { room } => state.worker_finish_room(*room),
            Self::PullEvent => state.pull_event().map(|_| ()),
        }
    }
}

/// Preserve the effects of the Pipeline's dropped-message sweep when an
/// awaited call reaches a documented wait condition. The await itself does
/// not complete, but the sweep occurs at the public-call boundary.
fn apply_blocking_call<F>(state: &mut State, wait_prefix: &str, call: F) -> Result<(), String>
where
    F: FnOnce(&mut State) -> Result<(), String>,
{
    let before = state.clone();
    match call(state) {
        Ok(()) => Ok(()),
        Err(error) if error.starts_with(wait_prefix) && *state != before => Ok(()),
        Err(error) => Err(error),
    }
}

#[derive(Clone)]
struct Node {
    state: State,
    path: Vec<String>,
    depth: usize,
}

#[derive(Default, Clone)]
struct WaitCounts {
    available: u64,
    worker: u64,
    caller: u64,
    consumer: u64,
    no_worker: u64,
}

impl WaitCounts {
    fn record(&mut self, class: WaitClass) {
        match class {
            WaitClass::Available => self.available += 1,
            WaitClass::WorkerCanProgress => self.worker += 1,
            WaitClass::CallerMustEndOrAbort | WaitClass::CallerMustReleaseRooms => self.caller += 1,
            WaitClass::ConsumerMustPullEvents => self.consumer += 1,
            WaitClass::NoWorkerCanProduceEvent => self.no_worker += 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExploreConfig {
    pub limits: Limits,
    pub max_depth: usize,
    pub max_states: usize,
}

impl Default for ExploreConfig {
    fn default() -> Self {
        Self {
            limits: Limits {
                messages: 2,
                rooms: 2,
            },
            max_depth: 8,
            max_states: 12_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExploreReport {
    pub states: usize,
    pub transitions: u64,
    pub invariant_checks: u64,
    pub max_active_messages: usize,
    pub max_held_or_inflight_rooms: usize,
    pub max_queued_events: usize,
    pub message_waits: WaitCountsPublic,
    pub room_waits: WaitCountsPublic,
    pub ready_waits: WaitCountsPublic,
    pub witness_message_caller: Option<Vec<String>>,
    pub witness_message_consumer: Option<Vec<String>>,
    pub witness_room_caller: Option<Vec<String>>,
    pub witness_ready_no_worker: Option<Vec<String>>,
    pub depth_limit_hit: bool,
    pub state_limit_hit: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WaitCountsPublic {
    pub available: u64,
    pub worker_can_progress: u64,
    pub caller_must_progress: u64,
    pub consumer_must_pull: u64,
    pub no_worker_can_produce: u64,
}

impl From<WaitCounts> for WaitCountsPublic {
    fn from(value: WaitCounts) -> Self {
        Self {
            available: value.available,
            worker_can_progress: value.worker,
            caller_must_progress: value.caller,
            consumer_must_pull: value.consumer,
            no_worker_can_produce: value.no_worker,
        }
    }
}

pub fn explore(config: ExploreConfig) -> Result<ExploreReport, String> {
    if config.max_depth == 0 || config.max_states == 0 {
        return Err("max-depth and max-states must both be greater than zero".into());
    }
    let initial = State::new(config.limits)?;
    let mut queue = VecDeque::from([Node {
        state: initial.clone(),
        path: Vec::new(),
        depth: 0,
    }]);
    let mut seen = BTreeSet::from([initial]);
    let mut transitions = 0u64;
    let mut invariant_checks = 0u64;
    let mut max_active_messages = 0usize;
    let mut max_held_or_inflight_rooms = 0usize;
    let mut max_queued_events = 0usize;
    let mut message_waits = WaitCounts::default();
    let mut room_waits = WaitCounts::default();
    let mut ready_waits = WaitCounts::default();
    let mut witness_message_caller = None;
    let mut witness_message_consumer = None;
    let mut witness_room_caller = None;
    let mut witness_ready_no_worker = None;
    let mut depth_limit_hit = false;
    let mut state_limit_hit = false;

    while let Some(node) = queue.pop_front() {
        node.state.check_invariants()?;
        invariant_checks += 1;
        max_active_messages = max_active_messages.max(node.state.messages.len());
        max_held_or_inflight_rooms = max_held_or_inflight_rooms.max(
            node.state
                .rooms
                .iter()
                .filter(|room| **room != RoomState::Free)
                .count(),
        );
        max_queued_events = max_queued_events.max(node.state.events.len());

        let mut observed_state = node.state.clone();
        observed_state.begin_api_call()?;
        let message_class = observed_state.message_wait_reason();
        let room_class = observed_state.room_wait_reason();
        if observed_state.messages.len() >= config.limits.messages {
            message_waits.record(message_class);
            match message_class {
                WaitClass::CallerMustEndOrAbort if witness_message_caller.is_none() => {
                    witness_message_caller = Some(node.path.clone());
                }
                WaitClass::ConsumerMustPullEvents if witness_message_consumer.is_none() => {
                    witness_message_consumer = Some(node.path.clone());
                }
                _ => {}
            }
            audit_message_wait(&observed_state, message_class)?;
        }
        let room_is_blocked = !observed_state.rooms.contains(&RoomState::Free);
        if room_is_blocked {
            room_waits.record(room_class);
            audit_room_wait(&observed_state, room_class)?;
        }
        if room_is_blocked
            && room_class == WaitClass::CallerMustReleaseRooms
            && witness_room_caller.is_none()
        {
            witness_room_caller = Some(node.path.clone());
        }

        let ready_is_blocked = observed_state.events.is_empty();
        if ready_is_blocked {
            let ready_class = observed_state.ready_wait_reason();
            ready_waits.record(ready_class);
            audit_ready_wait(&observed_state, ready_class)?;
            if ready_class == WaitClass::NoWorkerCanProduceEvent
                && witness_ready_no_worker.is_none()
            {
                witness_ready_no_worker = Some(node.path.clone());
            }
        }

        if node.depth >= config.max_depth {
            depth_limit_hit |= !enabled_actions(&node.state).is_empty();
            continue;
        }

        for action in enabled_actions(&node.state) {
            let mut next_state = node.state.clone();
            if action.apply(&mut next_state).is_err() {
                continue;
            }
            next_state.check_invariants()?;
            invariant_checks += 1;
            transitions += 1;
            if seen.contains(&next_state) {
                continue;
            }
            if seen.len() >= config.max_states {
                state_limit_hit = true;
                break;
            }
            seen.insert(next_state.clone());
            let mut path = node.path.clone();
            path.push(action.label());
            queue.push_back(Node {
                state: next_state,
                path,
                depth: node.depth + 1,
            });
        }
        if state_limit_hit {
            break;
        }
    }

    Ok(ExploreReport {
        states: seen.len(),
        transitions,
        invariant_checks,
        max_active_messages,
        max_held_or_inflight_rooms,
        max_queued_events,
        message_waits: message_waits.into(),
        room_waits: room_waits.into(),
        ready_waits: ready_waits.into(),
        witness_message_caller,
        witness_message_consumer,
        witness_room_caller,
        witness_ready_no_worker,
        depth_limit_hit,
        state_limit_hit,
    })
}

fn audit_message_wait(state: &State, class: WaitClass) -> Result<(), String> {
    if state.messages.len() < state.limits.messages {
        return Err("message wait audit invoked without a full message-record pool".into());
    }

    // A full event queue is a consumer-side bottleneck. Worker progress
    // cannot substitute for pulling an already queued event.
    if state.events.len() >= state.limits.messages {
        if class != WaitClass::ConsumerMustPullEvents {
            return Err(format!(
                "message wait oracle mismatch: full event pool requires consumer pull, class={class:?}"
            ));
        }
        return Ok(());
    }

    let worker_oracle = state.worker_oracle_can_free_message_slot();

    if worker_oracle && class != WaitClass::WorkerCanProgress {
        return Err(format!(
            "message wait oracle mismatch: worker can free a slot, class={class:?}"
        ));
    }

    if !worker_oracle && class != WaitClass::CallerMustEndOrAbort {
        return Err(format!(
            "message wait oracle mismatch: caller must end/abort, class={class:?}"
        ));
    }

    Ok(())
}
fn audit_room_wait(state: &State, class: WaitClass) -> Result<(), String> {
    let worker_oracle = state.worker_oracle_can_free_room();
    if worker_oracle && class != WaitClass::WorkerCanProgress {
        return Err(format!(
            "room wait oracle mismatch: worker can free a room, class={class:?}"
        ));
    }
    if !worker_oracle && class != WaitClass::CallerMustReleaseRooms {
        return Err(format!(
            "room wait oracle mismatch: caller holds all rooms, class={class:?}"
        ));
    }
    Ok(())
}

fn audit_ready_wait(state: &State, class: WaitClass) -> Result<(), String> {
    let worker_oracle = state.worker_oracle_can_produce_event();
    if worker_oracle && class != WaitClass::WorkerCanProgress {
        return Err(format!(
            "ready wait oracle mismatch: worker can produce an event, class={class:?}"
        ));
    }
    if !worker_oracle && class != WaitClass::NoWorkerCanProduceEvent {
        return Err(format!(
            "ready wait oracle mismatch: no worker can produce an event, class={class:?}"
        ));
    }
    Ok(())
}

fn enabled_actions(state: &State) -> Vec<Action> {
    let mut actions = Vec::new();
    if state.messages.len() < state.limits.messages
        || state.messages.values().any(|message| message.drop_pending)
    {
        // Opening a message is also a public Pipeline call. It observes any
        // dropped Message values before testing the message-record capacity.
        // Repeated tags are intentional; internal IDs remain independent.
        actions.push(Action::Open { tag: 7 });
    }
    if state.rooms.contains(&RoomState::Free)
        || state.messages.values().any(|message| message.drop_pending)
    {
        // room().await is a Pipeline call too. It observes dropped Messages
        // before it waits for room capacity.
        actions.push(Action::AcquireRoom);
    }
    for (room_id, room) in state.rooms.iter().enumerate() {
        match room {
            RoomState::Held => actions.push(Action::DropRoom { room: room_id }),
            RoomState::InFlight { .. } => actions.push(Action::FinishRoom { room: room_id }),
            RoomState::Free => {}
        }
    }
    for (message_id, message) in &state.messages {
        if message.finished_pending.is_none()
            && message.terminal.is_none()
            && !message.drop_pending
            && matches!(message.path, InputPath::None | InputPath::Rooms)
        {
            actions.push(Action::End {
                message: *message_id,
            });
            actions.push(Action::Abort {
                message: *message_id,
            });
            actions.push(Action::DropMessage {
                message: *message_id,
            });
        }
        if message.finished_pending.is_none()
            && message.terminal.is_none()
            && !message.drop_pending
            && message.path == InputPath::None
        {
            actions.push(Action::Owned {
                message: *message_id,
                token: message_id.saturating_add(100),
            });
            actions.push(Action::Lent {
                message: *message_id,
            });
            actions.push(Action::Batch {
                message: *message_id,
                batch: message_id.saturating_add(200),
                digests: message_id.saturating_add(300),
            });
        }
        if matches!(message.path, InputPath::None | InputPath::Rooms)
            && message.terminal.is_none()
            && !message.drop_pending
            && message.finished_pending.is_none()
        {
            for (room_id, _) in state
                .rooms
                .iter()
                .enumerate()
                .filter(|(_, room)| **room == RoomState::Held)
            {
                actions.push(Action::Commit {
                    message: *message_id,
                    room: room_id,
                });
            }
        }
        if message.direct_pending > 0 {
            actions.push(Action::StartDirect {
                message: *message_id,
            });
        }
        if message.direct_running > 0 {
            actions.push(Action::FinishDirect {
                message: *message_id,
            });
        }
    }
    for message_id in state.lent_guards.keys() {
        actions.push(Action::DropLentGuard {
            message: *message_id,
        });
    }
    if !state.events.is_empty() || state.messages.values().any(|message| message.drop_pending) {
        actions.push(Action::PullEvent);
    }
    actions.sort();
    actions.dedup();
    actions
}

pub fn render_json(report: &ExploreReport, config: &ExploreConfig) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"schema\": \"zapitra.explore.v2\",");
    let _ = writeln!(out, "  \"project\": \"ZAPITRA\",");
    let _ = writeln!(out, "  \"version\": \"{}\",", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(
        out,
        "  \"limits\": {{\"messages\": {}, \"rooms\": {}}},",
        config.limits.messages, config.limits.rooms
    );
    let _ = writeln!(out, "  \"max_depth\": {},", config.max_depth);
    let _ = writeln!(out, "  \"max_states\": {},", config.max_states);
    let _ = writeln!(out, "  \"states_explored\": {},", report.states);
    let _ = writeln!(out, "  \"transitions_checked\": {},", report.transitions);
    let _ = writeln!(out, "  \"invariant_checks\": {},", report.invariant_checks);
    let _ = writeln!(
        out,
        "  \"max_active_messages\": {},",
        report.max_active_messages
    );
    let _ = writeln!(
        out,
        "  \"max_rooms_in_use\": {},",
        report.max_held_or_inflight_rooms
    );
    let _ = writeln!(
        out,
        "  \"max_queued_events\": {},",
        report.max_queued_events
    );
    render_wait_counts(&mut out, "message_waits", &report.message_waits);
    render_wait_counts(&mut out, "room_waits", &report.room_waits);
    render_wait_counts(&mut out, "ready_waits", &report.ready_waits);
    let _ = writeln!(out, "  \"witnesses\": {{");
    render_witness(
        &mut out,
        "message_wait_caller_must_progress",
        &report.witness_message_caller,
        true,
    );
    render_witness(
        &mut out,
        "message_wait_consumer_must_pull",
        &report.witness_message_consumer,
        true,
    );
    render_witness(
        &mut out,
        "room_wait_caller_holds_all",
        &report.witness_room_caller,
        true,
    );
    render_witness(
        &mut out,
        "ready_no_worker_can_produce",
        &report.witness_ready_no_worker,
        false,
    );
    let _ = writeln!(out, "  }},");
    let _ = writeln!(out, "  \"depth_limit_hit\": {},", report.depth_limit_hit);
    let _ = writeln!(out, "  \"state_limit_hit\": {},", report.state_limit_hit);
    let verdict = if report.depth_limit_hit || report.state_limit_hit {
        "PASS_BOUNDED_MODEL_ORACLES_WITH_LIMIT"
    } else {
        "PASS_BOUNDED_MODEL_ORACLES"
    };
    let _ = writeln!(out, "  \"verdict\": \"{verdict}\",");
    let _ = writeln!(
        out,
        "  \"scope\": \"bounded reference-model exploration; not an upstream runtime test\""
    );
    let _ = writeln!(out, "}}");
    out
}

fn render_wait_counts(out: &mut String, name: &str, counts: &WaitCountsPublic) {
    let _ = writeln!(
        out,
        "  \"{name}\": {{\"available\": {}, \"worker_can_progress\": {}, \"caller_must_progress\": {}, \"consumer_must_pull\": {}, \"no_worker_can_produce\": {}}},",
        counts.available,
        counts.worker_can_progress,
        counts.caller_must_progress,
        counts.consumer_must_pull,
        counts.no_worker_can_produce,
    );
}

fn render_witness(out: &mut String, name: &str, path: &Option<Vec<String>>, comma: bool) {
    let text = match path {
        Some(items) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| format!("\"{}\"", escape_json(item)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        None => "null".into(),
    };
    let suffix = if comma { "," } else { "" };
    let _ = writeln!(out, "    \"{name}\": {text}{suffix}");
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub fn render_text(report: &ExploreReport, config: &ExploreConfig) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "ZAPITRA bounded state-space exploration");
    let _ = writeln!(
        out,
        "limits: messages={}, rooms={}",
        config.limits.messages, config.limits.rooms
    );
    let _ = writeln!(
        out,
        "depth limit: {} | state budget: {}",
        config.max_depth, config.max_states
    );
    let _ = writeln!(out, "states explored: {}", report.states);
    let _ = writeln!(out, "transitions checked: {}", report.transitions);
    let _ = writeln!(out, "invariant checks: {}", report.invariant_checks);
    let _ = writeln!(
        out,
        "peak active message records: {}",
        report.max_active_messages
    );
    let _ = writeln!(
        out,
        "peak rooms in use: {}",
        report.max_held_or_inflight_rooms
    );
    let _ = writeln!(out, "peak queued events: {}", report.max_queued_events);
    let _ = writeln!(
        out,
        "message wait classes: available={}, worker={}, caller={}, consumer={}",
        report.message_waits.available,
        report.message_waits.worker_can_progress,
        report.message_waits.caller_must_progress,
        report.message_waits.consumer_must_pull
    );
    let _ = writeln!(
        out,
        "room wait classes: available={}, worker={}, caller={}",
        report.room_waits.available,
        report.room_waits.worker_can_progress,
        report.room_waits.caller_must_progress
    );
    let _ = writeln!(
        out,
        "ready wait classes: worker={}, no-worker={}",
        report.ready_waits.worker_can_progress, report.ready_waits.no_worker_can_produce
    );
    let _ = writeln!(
        out,
        "depth limit hit: {} | state limit hit: {}",
        report.depth_limit_hit, report.state_limit_hit
    );
    let verdict = if report.depth_limit_hit || report.state_limit_hit {
        "PASS_BOUNDED_MODEL_ORACLES_WITH_LIMIT"
    } else {
        "PASS_BOUNDED_MODEL_ORACLES"
    };
    let _ = writeln!(out, "verdict: {verdict}");
    let _ = writeln!(
        out,
        "scope: bounded reference-model exploration; no upstream runtime implementation is exercised"
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_explorer_audits_wait_rules_and_invariants() {
        let config = ExploreConfig {
            limits: Limits {
                messages: 2,
                rooms: 2,
            },
            max_depth: 9,
            max_states: 20_000,
        };
        let report = explore(config).unwrap();
        assert!(report.states > 1);
        assert!(report.invariant_checks >= report.states as u64);
        assert!(report.message_waits.caller_must_progress > 0);
        assert!(report.message_waits.worker_can_progress > 0);
        assert!(report.message_waits.consumer_must_pull > 0);
        assert!(report.room_waits.caller_must_progress > 0);
        assert!(report.room_waits.worker_can_progress > 0);
        assert!(report.ready_waits.worker_can_progress > 0);
        assert!(report.ready_waits.no_worker_can_produce > 0);
    }

    #[test]
    fn event_queue_and_message_records_can_both_be_full() {
        let mut state = State::new(Limits {
            messages: 1,
            rooms: 1,
        })
        .unwrap();
        let a = state.open_message(7).unwrap();
        state.end_message(a).unwrap();
        let b = state.open_message(7).unwrap();
        state.end_message(b).unwrap();
        assert_eq!(state.events.len(), 1);
        assert_eq!(state.messages.len(), 1);
        assert_eq!(
            state.message_wait_reason(),
            WaitClass::ConsumerMustPullEvents
        );
    }

    #[test]
    fn opening_a_message_sweeps_a_dropped_message_before_capacity_check() {
        let mut state = State::new(Limits {
            messages: 1,
            rooms: 1,
        })
        .unwrap();
        let dropped = state.open_message(1).unwrap();
        state.drop_message(dropped).unwrap();
        let open = enabled_actions(&state)
            .into_iter()
            .find(|action| matches!(action, Action::Open { .. }))
            .expect("Open must be enabled because it observes the dropped token");
        open.apply(&mut state).unwrap();
        assert_eq!(state.messages.len(), 1);
        assert_eq!(
            state.events.front().map(|event| event.kind),
            Some(crate::model::EventKind::Aborted)
        );
        assert_eq!(state.messages.values().next().unwrap().tag, 7);
        state.check_invariants().unwrap();
    }
}
