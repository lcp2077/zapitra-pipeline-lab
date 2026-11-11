//! Executable reference model for the public BLAKE3-Servil Pipeline design.
//!
//! This module models API contracts. It does not hash bytes or claim to be
//! an implementation of the upstream Pipeline runtime.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PIPELINE_ID: AtomicU64 = AtomicU64::new(1);

pub const ROOM_LEN: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Limits {
    pub messages: usize,
    pub rooms: usize,
}

impl Limits {
    pub fn validate(self) -> Result<Self, String> {
        if self.messages == 0 {
            return Err("the model requires limits.messages >= 1".into());
        }
        if self.rooms == 0 {
            return Err("the model requires limits.rooms >= 1".into());
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum InputPath {
    None,
    Rooms,
    Owned,
    Lent,
    Batch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum EventKind {
    Hashed,
    Aborted,
    Batch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Terminal {
    End,
    Abort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum LentGuardState {
    Active,
    EventMaterialized,
    EventDelivered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RoomState {
    Free,
    Held,
    InFlight { message_id: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EventRecord {
    pub message_id: u16,
    pub tag: u8,
    pub finish_order: u64,
    pub kind: EventKind,
    pub owner_token: Option<u16>,
    pub batch_token: Option<(u16, u16)>,
}

/// Opaque model token for an open message. It carries the originating Pipeline ID.
#[derive(Debug, Eq, PartialEq)]
pub struct MessageToken {
    pipeline_id: u64,
    message_id: u16,
}

impl MessageToken {
    pub fn message_id(&self) -> u16 {
        self.message_id
    }
}

/// Opaque model token for an acquired room. It carries the originating Pipeline ID.
#[derive(Debug, Eq, PartialEq)]
pub struct RoomToken {
    pipeline_id: u64,
    room_id: usize,
}

impl RoomToken {
    pub fn room_id(&self) -> usize {
        self.room_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct MessageRecord {
    pub id: u16,
    /// Tags are payload returned to the caller. They are deliberately allowed
    /// to repeat; only `id` identifies a modelled message internally.
    pub tag: u8,
    pub path: InputPath,
    pub terminal: Option<Terminal>,
    pub drop_pending: bool,
    pub in_flight_rooms: usize,
    pub direct_pending: usize,
    pub direct_running: usize,
    pub lent_cancelled: bool,
    pub owner_token: Option<u16>,
    pub batch_token: Option<(u16, u16)>,
    /// A completed result waiting for event-queue capacity. It remains in the
    /// message-record pool until a distinct event slot becomes available.
    pub finished_pending: Option<EventRecord>,
}

impl MessageRecord {
    fn new(id: u16, tag: u8) -> Self {
        Self {
            id,
            tag,
            path: InputPath::None,
            terminal: None,
            drop_pending: false,
            in_flight_rooms: 0,
            direct_pending: 0,
            direct_running: 0,
            lent_cancelled: false,
            owner_token: None,
            batch_token: None,
            finished_pending: None,
        }
    }

    pub fn is_finished_pending(&self) -> bool {
        self.finished_pending.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitClass {
    Available,
    WorkerCanProgress,
    CallerMustEndOrAbort,
    ConsumerMustPullEvents,
    CallerMustReleaseRooms,
    NoWorkerCanProduceEvent,
}

impl WaitClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "capacity_available",
            Self::WorkerCanProgress => "worker_can_free_capacity",
            Self::CallerMustEndOrAbort => "caller_must_end_or_abort_an_open_message",
            Self::ConsumerMustPullEvents => "consumer_must_pull_finished_events",
            Self::CallerMustReleaseRooms => "caller_holds_all_rooms_uncommitted",
            Self::NoWorkerCanProduceEvent => "no_worker_can_produce_an_event",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct State {
    pipeline_id: u64,
    pub limits: Limits,
    pub next_message_id: u16,
    pub next_finish_order: u64,
    pub messages: BTreeMap<u16, MessageRecord>,
    pub rooms: Vec<RoomState>,
    pub events: VecDeque<EventRecord>,
    pub lent_guards: BTreeMap<u16, LentGuardState>,
}

impl State {
    pub fn new(limits: Limits) -> Result<Self, String> {
        let limits = limits.validate()?;
        Ok(Self {
            pipeline_id: NEXT_PIPELINE_ID.fetch_add(1, Ordering::Relaxed),
            limits,
            next_message_id: 0,
            next_finish_order: 0,
            messages: BTreeMap::new(),
            rooms: vec![RoomState::Free; limits.rooms],
            events: VecDeque::new(),
            lent_guards: BTreeMap::new(),
        })
    }

    /// Return the identity used by this model instance for token provenance.
    pub fn pipeline_id(&self) -> u64 {
        self.pipeline_id
    }

    /// Runtime-contract probe for a `Message` or `Room` passed to a Pipeline
    /// instance other than the one that created it.
    pub fn validate_token_origin(&self, token_pipeline_id: u64) -> Result<(), String> {
        if token_pipeline_id != self.pipeline_id {
            return Err("Message or Room belongs to a different Pipeline instance".into());
        }
        Ok(())
    }

    /// Open a message and return a non-cloneable token tied to this model instance.
    pub fn open_message_token(&mut self, tag: u8) -> Result<MessageToken, String> {
        let message_id = self.open_message(tag)?;
        Ok(MessageToken {
            pipeline_id: self.pipeline_id,
            message_id,
        })
    }

    /// Acquire a room and return a token tied to this model instance.
    pub fn acquire_room_token(&mut self) -> Result<RoomToken, String> {
        let room_id = self.acquire_room()?;
        Ok(RoomToken {
            pipeline_id: self.pipeline_id,
            room_id,
        })
    }

    /// Commit a room token to a message token. The room token is consumed.
    pub fn commit_tokens(
        &mut self,
        message: &mut MessageToken,
        room: RoomToken,
        written: usize,
    ) -> Result<(), String> {
        self.validate_token_origin(message.pipeline_id)?;
        self.validate_token_origin(room.pipeline_id)?;
        self.commit(message.message_id, room.room_id, written)
    }

    /// Consume a message token by ending the message.
    pub fn end_token(&mut self, message: MessageToken) -> Result<(), String> {
        self.validate_token_origin(message.pipeline_id)?;
        self.end_message(message.message_id)
    }

    /// Consume a message token by aborting the message.
    pub fn abort_token(&mut self, message: MessageToken) -> Result<(), String> {
        self.validate_token_origin(message.pipeline_id)?;
        self.abort_message(message.message_id)
    }

    /// Drop a message token. The abort is observed at the next Pipeline call.
    pub fn drop_message_token(&mut self, message: MessageToken) -> Result<(), String> {
        self.validate_token_origin(message.pipeline_id)?;
        self.drop_message(message.message_id)
    }

    /// Drop an uncommitted room token.
    pub fn drop_room_token(&mut self, room: RoomToken) -> Result<(), String> {
        self.validate_token_origin(room.pipeline_id)?;
        self.drop_room(room.room_id)
    }

    /// Model `message(tag, length_hint).await` after the API's next-call
    /// dropped-message sweep has run.
    pub fn open_message(&mut self, tag: u8) -> Result<u16, String> {
        self.begin_api_call()?;
        if self.messages.len() >= self.limits.messages {
            return Err(format!(
                "message wait: {}",
                self.message_wait_class().as_str()
            ));
        }
        let id = self.next_message_id;
        self.next_message_id = self
            .next_message_id
            .checked_add(1)
            .ok_or_else(|| "model message identifier exhausted".to_string())?;
        self.messages.insert(id, MessageRecord::new(id, tag));
        self.check_invariants()?;
        Ok(id)
    }

    /// Model `room().await`. A room is acquired independently of any message.
    pub fn acquire_room(&mut self) -> Result<usize, String> {
        self.begin_api_call()?;
        let Some(index) = self.rooms.iter().position(|room| *room == RoomState::Free) else {
            return Err(format!("room wait: {}", self.room_wait_class().as_str()));
        };
        self.rooms[index] = RoomState::Held;
        self.check_invariants()?;
        Ok(index)
    }

    /// Model `commit(&mut message, room, written)`. The room may have been
    /// acquired before the caller knew which message would receive its bytes.
    pub fn commit(
        &mut self,
        message_id: u16,
        room_id: usize,
        written: usize,
    ) -> Result<(), String> {
        self.begin_api_call()?;
        if written > ROOM_LEN {
            return Err(format!(
                "commit rejected: written={written} exceeds ROOM_LEN={ROOM_LEN}"
            ));
        }
        let room = self
            .rooms
            .get(room_id)
            .copied()
            .ok_or_else(|| format!("unknown room {room_id}"))?;
        if room != RoomState::Held {
            return Err(format!("room {room_id} is not held and uncommitted"));
        }
        let message = self
            .messages
            .get_mut(&message_id)
            .ok_or_else(|| format!("message {message_id} is unknown or already consumed"))?;
        if message.finished_pending.is_some() || message.terminal.is_some() || message.drop_pending
        {
            return Err("message no longer accepts committed rooms".into());
        }
        if !matches!(message.path, InputPath::None | InputPath::Rooms) {
            return Err("a message may receive bytes through exactly one input path".into());
        }
        message.path = InputPath::Rooms;
        message.in_flight_rooms += 1;
        self.rooms[room_id] = RoomState::InFlight { message_id };
        self.check_invariants()
    }

    /// Dropping an uncommitted `Room` returns it to the pool.
    pub fn drop_room(&mut self, room_id: usize) -> Result<(), String> {
        let room = self
            .rooms
            .get_mut(room_id)
            .ok_or_else(|| format!("unknown room {room_id}"))?;
        if *room != RoomState::Held {
            return Err("only a held, uncommitted room can be dropped directly".into());
        }
        *room = RoomState::Free;
        self.check_invariants()
    }

    /// Model `end(message)`. The message token is consumed by this operation.
    pub fn end_message(&mut self, message_id: u16) -> Result<(), String> {
        self.begin_api_call()?;
        self.set_terminal(message_id, Terminal::End)
    }

    /// Model `abort(message)`. The message token is consumed by this operation.
    pub fn abort_message(&mut self, message_id: u16) -> Result<(), String> {
        self.begin_api_call()?;
        self.set_terminal(message_id, Terminal::Abort)
    }

    fn set_terminal(&mut self, message_id: u16, terminal: Terminal) -> Result<(), String> {
        let ready = {
            let message = self
                .messages
                .get_mut(&message_id)
                .ok_or_else(|| format!("message {message_id} is unknown or already consumed"))?;
            if message.finished_pending.is_some()
                || message.terminal.is_some()
                || message.drop_pending
            {
                return Err("message is already terminal or dropped".into());
            }
            if !matches!(message.path, InputPath::None | InputPath::Rooms) {
                return Err(
                    "end/abort consumes a message; direct hand-off already consumed it".into(),
                );
            }
            message.terminal = Some(terminal);
            message.in_flight_rooms == 0
        };
        if ready {
            self.finish_message(
                message_id,
                match terminal {
                    Terminal::End => EventKind::Hashed,
                    Terminal::Abort => EventKind::Aborted,
                },
            )?;
        }
        self.check_invariants()
    }

    /// Model the `owned(message, owner)` transfer. Work is asynchronous in the
    /// model; a worker must start and finish it before an event is produced.
    pub fn owned(&mut self, message_id: u16, owner_token: u16) -> Result<(), String> {
        self.begin_api_call()?;
        let message = self
            .messages
            .get_mut(&message_id)
            .ok_or_else(|| format!("message {message_id} is unknown or already consumed"))?;
        Self::ensure_direct_input_allowed(message)?;
        message.path = InputPath::Owned;
        message.terminal = Some(Terminal::End);
        message.owner_token = Some(owner_token);
        message.direct_pending = 1;
        self.check_invariants()
    }

    /// Model `unsafe lent(message, bytes)`. The symbolic guard stays with the
    /// caller; it is not carried by `Event::Hashed`.
    pub fn lent(&mut self, message_id: u16) -> Result<(), String> {
        self.begin_api_call()?;
        let message = self
            .messages
            .get_mut(&message_id)
            .ok_or_else(|| format!("message {message_id} is unknown or already consumed"))?;
        Self::ensure_direct_input_allowed(message)?;
        message.path = InputPath::Lent;
        message.direct_pending = 2;
        self.lent_guards.insert(message_id, LentGuardState::Active);
        self.check_invariants()
    }

    /// Drop a `Lent` guard. Before event materialization this cancels unstarted
    /// work and waits for active readers. After materialization it only releases
    /// the loan, even if the consumer has not pulled the event yet.
    pub fn drop_lent_guard(&mut self, message_id: u16) -> Result<(), String> {
        let status = self
            .lent_guards
            .remove(&message_id)
            .ok_or_else(|| format!("no live Lent guard for message {message_id}"))?;
        match status {
            LentGuardState::EventMaterialized | LentGuardState::EventDelivered => {
                // Hashing finished before the guard was dropped. The event may
                // still be queued, be waiting for queue capacity, or already
                // have been returned to the caller.
            }
            LentGuardState::Active => {
                let finish_now = {
                    let message = self
                        .messages
                        .get_mut(&message_id)
                        .ok_or_else(|| "active Lent guard has no message record".to_string())?;
                    message.lent_cancelled = true;
                    message.terminal = Some(Terminal::Abort);
                    message.direct_pending = 0;
                    message.direct_running == 0
                };
                if finish_now {
                    self.finish_message(message_id, EventKind::Aborted)?;
                }
            }
        }
        self.check_invariants()
    }

    /// Model `batch(message, Batch<'static>, digests)`. The two storage tokens
    /// return together in the Batch event.
    pub fn batch(
        &mut self,
        message_id: u16,
        batch_token: u16,
        digest_storage_token: u16,
    ) -> Result<(), String> {
        self.begin_api_call()?;
        let message = self
            .messages
            .get_mut(&message_id)
            .ok_or_else(|| format!("message {message_id} is unknown or already consumed"))?;
        Self::ensure_direct_input_allowed(message)?;
        message.path = InputPath::Batch;
        message.terminal = Some(Terminal::End);
        message.batch_token = Some((batch_token, digest_storage_token));
        message.direct_pending = 1;
        self.check_invariants()
    }

    fn ensure_direct_input_allowed(message: &MessageRecord) -> Result<(), String> {
        if message.finished_pending.is_some()
            || message.terminal.is_some()
            || message.drop_pending
            || message.path != InputPath::None
            || message.in_flight_rooms != 0
        {
            return Err(
                "owned/lent/batch requires an open message with no other byte source".into(),
            );
        }
        Ok(())
    }

    /// Drop a `Message` value. The abort is observed by the next public
    /// Pipeline call, as specified by the design.
    pub fn drop_message(&mut self, message_id: u16) -> Result<(), String> {
        let message = self
            .messages
            .get_mut(&message_id)
            .ok_or_else(|| format!("message {message_id} is unknown or already consumed"))?;
        if message.path != InputPath::None && message.path != InputPath::Rooms {
            return Err(
                "direct hand-off consumes Message; it cannot be dropped as an open Message".into(),
            );
        }
        if message.terminal.is_some() || message.finished_pending.is_some() || message.drop_pending
        {
            return Err("message is already terminal or dropped".into());
        }
        message.drop_pending = true;
        self.check_invariants()
    }

    /// A public API call begins by noticing any dropped Message values.
    pub(crate) fn begin_api_call(&mut self) -> Result<(), String> {
        let dropped: Vec<u16> = self
            .messages
            .iter()
            .filter_map(|(id, message)| message.drop_pending.then_some(*id))
            .collect();
        for id in dropped {
            let finish_now = if let Some(message) = self.messages.get_mut(&id) {
                message.drop_pending = false;
                message.terminal = Some(Terminal::Abort);
                message.in_flight_rooms == 0
            } else {
                false
            };
            if finish_now {
                self.finish_message(id, EventKind::Aborted)?;
            }
        }
        self.check_invariants()
    }

    /// Worker starts one unit from an owned, lent, or batch message.
    pub fn worker_start_direct(&mut self, message_id: u16) -> Result<(), String> {
        let message = self
            .messages
            .get_mut(&message_id)
            .ok_or_else(|| format!("unknown or already completed message {message_id}"))?;
        if message.finished_pending.is_some() || message.direct_pending == 0 {
            return Err("message has no direct work waiting to start".into());
        }
        if message.path == InputPath::Lent && message.lent_cancelled {
            return Err("cancelled Lent work cannot start another reader".into());
        }
        message.direct_pending -= 1;
        message.direct_running += 1;
        self.check_invariants()
    }

    /// Worker finishes one direct-work unit. Lent cancellation is reported
    /// only after every already-running reader has returned.
    pub fn worker_finish_direct(&mut self, message_id: u16) -> Result<(), String> {
        let (complete, kind) = {
            let message = self
                .messages
                .get_mut(&message_id)
                .ok_or_else(|| format!("unknown or already completed message {message_id}"))?;
            if message.direct_running == 0 {
                return Err("message has no direct worker currently running".into());
            }
            message.direct_running -= 1;
            let complete = message.direct_running == 0 && message.direct_pending == 0;
            let kind = if message.path == InputPath::Lent && message.lent_cancelled {
                EventKind::Aborted
            } else {
                match message.path {
                    InputPath::Batch => EventKind::Batch,
                    InputPath::Owned | InputPath::Lent => EventKind::Hashed,
                    _ => return Err("direct worker attached to a non-direct input path".into()),
                }
            };
            if complete {
                message.terminal = Some(if kind == EventKind::Aborted {
                    Terminal::Abort
                } else {
                    Terminal::End
                });
            }
            (complete, kind)
        };
        if complete {
            self.finish_message(message_id, kind)?;
        }
        self.check_invariants()
    }

    /// Worker hashes one committed room. The room returns to the pool as soon
    /// as its bytes have been hashed, independently of event consumption.
    pub fn worker_finish_room(&mut self, room_id: usize) -> Result<(), String> {
        let room = self
            .rooms
            .get(room_id)
            .copied()
            .ok_or_else(|| format!("unknown room {room_id}"))?;
        let RoomState::InFlight { message_id } = room else {
            return Err("room is not in flight".into());
        };
        self.rooms[room_id] = RoomState::Free;
        let terminal_to_finish = {
            let message = self
                .messages
                .get_mut(&message_id)
                .ok_or_else(|| "in-flight room has no active message record".to_string())?;
            message.in_flight_rooms = message
                .in_flight_rooms
                .checked_sub(1)
                .ok_or_else(|| "in-flight room counter underflow".to_string())?;
            if message.in_flight_rooms == 0 {
                message.terminal
            } else {
                None
            }
        };
        if let Some(terminal) = terminal_to_finish {
            self.finish_message(
                message_id,
                match terminal {
                    Terminal::End => EventKind::Hashed,
                    Terminal::Abort => EventKind::Aborted,
                },
            )?;
        }
        self.check_invariants()
    }

    fn finish_message(&mut self, message_id: u16, kind: EventKind) -> Result<(), String> {
        let (event, path) = {
            let message = self
                .messages
                .get(&message_id)
                .ok_or_else(|| format!("cannot finish missing message {message_id}"))?;
            if message.finished_pending.is_some() {
                return Err(format!(
                    "message {message_id} already has a finished result"
                ));
            }
            if message.in_flight_rooms != 0
                || message.direct_pending != 0
                || message.direct_running != 0
            {
                return Err(format!("message {message_id} still has worker-owned input"));
            }
            let event = EventRecord {
                message_id,
                tag: message.tag,
                finish_order: self.next_finish_order,
                kind,
                owner_token: if kind == EventKind::Aborted || kind == EventKind::Hashed {
                    message.owner_token
                } else {
                    None
                },
                batch_token: if kind == EventKind::Batch {
                    message.batch_token
                } else {
                    None
                },
            };
            (event, message.path)
        };
        self.next_finish_order = self
            .next_finish_order
            .checked_add(1)
            .ok_or_else(|| "finish-order counter exhausted".to_string())?;

        if path == InputPath::Lent && kind == EventKind::Hashed {
            // Option 2 (limited Lent): all readers have finished before this
            // event is created. The caller's borrow may be released now, even
            // if bounded event storage delays the consumer's next_event call.
            self.lent_guards
                .insert(message_id, LentGuardState::EventMaterialized);
        }

        if self.events.len() < self.limits.messages {
            self.messages.remove(&message_id);
            self.events.push_back(event);
        } else {
            // The public design gives completed events their own bounded pool.
            // It does not spell out the storage location when that pool is full.
            // This model retains the completed result in the originating record.
            self.messages
                .get_mut(&message_id)
                .expect("message checked above")
                .finished_pending = Some(event);
        }
        Ok(())
    }

    /// Model `next_event()`: it also starts with the dropped-message sweep.
    pub fn pull_event(&mut self) -> Result<Option<EventRecord>, String> {
        self.begin_api_call()?;
        let event = self.events.pop_front();
        if let Some(item) = &event {
            match self.lent_guards.get_mut(&item.message_id) {
                Some(status) if *status == LentGuardState::EventMaterialized => {
                    *status = LentGuardState::EventDelivered;
                }
                _ => {}
            }
            self.promote_finished_pending()?;
        }
        self.check_invariants()?;
        Ok(event)
    }

    fn promote_finished_pending(&mut self) -> Result<(), String> {
        while self.events.len() < self.limits.messages {
            let next = self
                .messages
                .iter()
                .filter_map(|(id, message)| {
                    message
                        .finished_pending
                        .as_ref()
                        .map(|event| (event.finish_order, *id))
                })
                .min();
            let Some((_, id)) = next else {
                break;
            };
            let mut message = self
                .messages
                .remove(&id)
                .ok_or_else(|| "finished-pending message disappeared".to_string())?;
            let event = message
                .finished_pending
                .take()
                .ok_or_else(|| "finished-pending result disappeared".to_string())?;
            self.events.push_back(event);
        }
        Ok(())
    }

    pub fn message_wait_class(&self) -> WaitClass {
        let mut state = self.clone();
        if state.begin_api_call().is_err() {
            return WaitClass::NoWorkerCanProduceEvent;
        }
        if state.messages.len() < state.limits.messages {
            return WaitClass::Available;
        }
        if state.events.len() >= state.limits.messages {
            return WaitClass::ConsumerMustPullEvents;
        }
        if state.worker_oracle_can_free_message_slot() {
            WaitClass::WorkerCanProgress
        } else {
            WaitClass::CallerMustEndOrAbort
        }
    }

    pub fn room_wait_class(&self) -> WaitClass {
        let mut state = self.clone();
        if state.begin_api_call().is_err() {
            return WaitClass::NoWorkerCanProduceEvent;
        }
        if state.rooms.contains(&RoomState::Free) {
            return WaitClass::Available;
        }
        if state.worker_oracle_can_free_room() {
            WaitClass::WorkerCanProgress
        } else {
            WaitClass::CallerMustReleaseRooms
        }
    }

    pub fn ready_wait_class(&self) -> WaitClass {
        let mut state = self.clone();
        if state.begin_api_call().is_err() {
            return WaitClass::NoWorkerCanProduceEvent;
        }
        if !state.events.is_empty() {
            return WaitClass::Available;
        }
        if state.worker_oracle_can_produce_event() {
            WaitClass::WorkerCanProgress
        } else {
            WaitClass::NoWorkerCanProduceEvent
        }
    }

    /// Independent worker-only reachability oracle. It explores all legal
    /// worker schedules without allowing caller actions or event pulls.
    pub fn worker_oracle_can_free_message_slot(&self) -> bool {
        if self.messages.len() < self.limits.messages {
            return true;
        }
        let mut queue = VecDeque::from([self.clone()]);
        let mut seen = BTreeSet::new();
        while let Some(state) = queue.pop_front() {
            if !seen.insert(state.clone()) {
                continue;
            }
            if state.messages.len() < self.limits.messages {
                return true;
            }
            for next in state.worker_successors() {
                queue.push_back(next);
            }
        }
        false
    }

    pub fn worker_oracle_can_free_room(&self) -> bool {
        if self.rooms.contains(&RoomState::Free) {
            return true;
        }
        let mut queue = VecDeque::from([self.clone()]);
        let mut seen = BTreeSet::new();
        while let Some(state) = queue.pop_front() {
            if !seen.insert(state.clone()) {
                continue;
            }
            if state.rooms.contains(&RoomState::Free) {
                return true;
            }
            for next in state.worker_successors() {
                queue.push_back(next);
            }
        }
        false
    }

    pub fn worker_oracle_can_produce_event(&self) -> bool {
        if !self.events.is_empty() {
            return true;
        }
        let mut queue = VecDeque::from([self.clone()]);
        let mut seen = BTreeSet::new();
        while let Some(state) = queue.pop_front() {
            if !seen.insert(state.clone()) {
                continue;
            }
            if !state.events.is_empty() {
                return true;
            }
            for next in state.worker_successors() {
                queue.push_back(next);
            }
        }
        false
    }

    fn worker_successors(&self) -> Vec<Self> {
        let mut successors = Vec::new();
        for (room_id, room) in self.rooms.iter().enumerate() {
            let mut next = self.clone();
            if matches!(room, RoomState::InFlight { .. })
                && next.worker_finish_room(room_id).is_ok()
            {
                successors.push(next);
            }
        }
        for (id, message) in &self.messages {
            let mut start = self.clone();
            if message.direct_pending > 0 && start.worker_start_direct(*id).is_ok() {
                successors.push(start);
            }
            let mut finish = self.clone();
            if message.direct_running > 0 && finish.worker_finish_direct(*id).is_ok() {
                successors.push(finish);
            }
        }
        successors
    }

    pub fn check_invariants(&self) -> Result<(), String> {
        if self.messages.len() > self.limits.messages {
            return Err("message-record pool exceeds limits.messages".into());
        }
        if self.events.len() > self.limits.messages {
            return Err("finished-event pool exceeds limits.messages".into());
        }
        if self.rooms.len() != self.limits.rooms {
            return Err("room pool length differs from limits.rooms".into());
        }

        let mut room_counts = BTreeMap::<u16, usize>::new();
        for room in &self.rooms {
            if let RoomState::InFlight { message_id } = room {
                *room_counts.entry(*message_id).or_default() += 1;
                let message = self.messages.get(message_id).ok_or_else(|| {
                    format!("in-flight room refers to absent message {message_id}")
                })?;
                if message.path != InputPath::Rooms {
                    return Err(format!(
                        "in-flight room is attached to non-room message {message_id}"
                    ));
                }
            }
        }

        let mut pending_orders = Vec::new();
        for (id, message) in &self.messages {
            if *id != message.id {
                return Err("message map key differs from internal message ID".into());
            }
            let actual_rooms = room_counts.get(id).copied().unwrap_or(0);
            if actual_rooms != message.in_flight_rooms {
                return Err(format!(
                    "message {id} has an inconsistent in-flight room count"
                ));
            }
            if message.path != InputPath::Rooms && message.in_flight_rooms != 0 {
                return Err(format!("non-room message {id} owns committed rooms"));
            }
            if message.path != InputPath::Owned && message.owner_token.is_some() {
                return Err(format!("non-owned message {id} carries an owner token"));
            }
            if message.path != InputPath::Batch && message.batch_token.is_some() {
                return Err(format!("non-batch message {id} carries batch storage"));
            }
            if message.path == InputPath::Lent
                && !message.lent_cancelled
                && message.finished_pending.is_none()
                && !self.lent_guards.contains_key(id)
            {
                return Err(format!(
                    "unfinished Lent message {id} has no live caller guard"
                ));
            }
            if message.path != InputPath::Lent && self.lent_guards.contains_key(id) {
                return Err(format!("non-Lent message {id} has a Lent guard"));
            }
            if let Some(finished) = &message.finished_pending {
                if message.in_flight_rooms != 0
                    || message.direct_pending != 0
                    || message.direct_running != 0
                {
                    return Err(format!(
                        "finished-pending message {id} still has worker work"
                    ));
                }
                if message.terminal.is_none() {
                    return Err(format!(
                        "finished-pending message {id} has no terminal outcome"
                    ));
                }
                pending_orders.push(finished.finish_order);
            }
            if message.path == InputPath::Rooms
                && message.terminal.is_some()
                && message.drop_pending
            {
                return Err(format!("terminal room message {id} is also drop-pending"));
            }
            if message.path == InputPath::Lent
                && message.lent_cancelled
                && message.direct_pending != 0
            {
                return Err(format!(
                    "cancelled Lent message {id} still has unstarted work"
                ));
            }
        }

        let mut previous = None;
        let mut event_ids = BTreeSet::new();
        for event in &self.events {
            if !event_ids.insert(event.message_id) {
                return Err(format!(
                    "message {} has more than one queued event",
                    event.message_id
                ));
            }
            if previous.is_some_and(|last| event.finish_order <= last) {
                return Err("event queue is not ordered by completion time".into());
            }
            previous = Some(event.finish_order);
            if self.messages.contains_key(&event.message_id) {
                return Err(format!(
                    "message {} exists both as a record and an event",
                    event.message_id
                ));
            }
        }
        pending_orders.sort_unstable();
        if previous
            .is_some_and(|last_queued| pending_orders.iter().any(|order| *order <= last_queued))
        {
            return Err("a pending finished result precedes an already queued event".into());
        }
        if self.events.len() < self.limits.messages && !pending_orders.is_empty() {
            return Err("finished-pending records exist while event capacity is available".into());
        }

        for (id, status) in &self.lent_guards {
            match status {
                LentGuardState::Active => {
                    let message = self
                        .messages
                        .get(id)
                        .ok_or_else(|| format!("active Lent guard {id} has no message"))?;
                    if message.path != InputPath::Lent || message.lent_cancelled {
                        return Err(format!("active Lent guard {id} has invalid message state"));
                    }
                }
                LentGuardState::EventMaterialized => {
                    let has_result = self.events.iter().any(|event| event.message_id == *id)
                        || self
                            .messages
                            .get(id)
                            .is_some_and(|message| message.finished_pending.is_some());
                    if !has_result {
                        return Err(format!(
                            "materialized Lent guard {id} has no queued or pending event"
                        ));
                    }
                }
                LentGuardState::EventDelivered => {
                    if self.messages.contains_key(id)
                        || self.events.iter().any(|event| event.message_id == *id)
                    {
                        return Err(format!(
                            "delivered Lent guard {id} still has an undelivered message event"
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn message_wait_reason(&self) -> WaitClass {
        if self.messages.len() < self.limits.messages {
            WaitClass::Available
        } else if self.events.len() >= self.limits.messages {
            WaitClass::ConsumerMustPullEvents
        } else if self.worker_oracle_can_free_message_slot() {
            WaitClass::WorkerCanProgress
        } else {
            WaitClass::CallerMustEndOrAbort
        }
    }

    pub fn room_wait_reason(&self) -> WaitClass {
        if self.rooms.contains(&RoomState::Free) {
            WaitClass::Available
        } else if self.worker_oracle_can_free_room() {
            WaitClass::WorkerCanProgress
        } else {
            WaitClass::CallerMustReleaseRooms
        }
    }

    pub fn ready_wait_reason(&self) -> WaitClass {
        if !self.events.is_empty() {
            WaitClass::Available
        } else if self.worker_oracle_can_produce_event() {
            WaitClass::WorkerCanProgress
        } else {
            WaitClass::NoWorkerCanProduceEvent
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(messages: usize, rooms: usize) -> State {
        State::new(Limits { messages, rooms }).unwrap()
    }

    #[test]
    fn duplicate_tags_are_valid_and_internal_ids_are_distinct() {
        let mut state = model(2, 1);
        let a = state.open_message(7).unwrap();
        let b = state.open_message(7).unwrap();
        assert_ne!(a, b);
        state.end_message(a).unwrap();
        state.end_message(b).unwrap();
        let first = state.pull_event().unwrap().unwrap();
        let second = state.pull_event().unwrap().unwrap();
        assert_eq!(first.tag, 7);
        assert_eq!(second.tag, 7);
        assert_ne!(first.message_id, second.message_id);
        state.check_invariants().unwrap();
    }

    #[test]
    fn message_and_event_pools_are_independent() {
        let mut state = model(1, 1);
        let a = state.open_message(1).unwrap();
        state.owned(a, 10).unwrap();
        assert_eq!(state.events.len(), 0);
        state.worker_start_direct(a).unwrap();
        state.worker_finish_direct(a).unwrap();
        assert_eq!(state.events.len(), 1);
        assert_eq!(state.messages.len(), 0);
        let b = state.open_message(2).unwrap();
        assert_eq!(
            state.message_wait_reason(),
            WaitClass::ConsumerMustPullEvents
        );
        assert_eq!(state.events.len(), 1);
        assert_eq!(state.messages.len(), 1);
        state.end_message(b).unwrap();
        assert_eq!(
            state.message_wait_reason(),
            WaitClass::ConsumerMustPullEvents
        );
        let first = state.pull_event().unwrap().unwrap();
        assert_eq!(first.owner_token, Some(10));
        let second = state.pull_event().unwrap().unwrap();
        assert_eq!(second.tag, 2);
        state.check_invariants().unwrap();
    }

    #[test]
    fn event_full_wait_stops_even_if_terminal_worker_work_exists() {
        let mut state = model(1, 1);
        let first = state.open_message(1).unwrap();
        state.end_message(first).unwrap();
        let second = state.open_message(2).unwrap();
        let room = state.acquire_room().unwrap();
        state.commit(second, room, 32).unwrap();
        state.end_message(second).unwrap();
        assert_eq!(
            state.message_wait_reason(),
            WaitClass::ConsumerMustPullEvents
        );
        state.worker_finish_room(room).unwrap();
        assert_eq!(state.rooms[room], RoomState::Free);
        assert_eq!(
            state.messages.len(),
            1,
            "the finished result retains its message record while event storage is full"
        );
        assert_eq!(state.pull_event().unwrap().unwrap().tag, 1);
        assert_eq!(
            state.messages.len(),
            0,
            "promotion into the event pool releases the message record"
        );
        assert_eq!(state.pull_event().unwrap().unwrap().tag, 2);
        state.check_invariants().unwrap();
    }

    #[test]
    fn room_wait_is_reclassified_after_a_worker_wakes_it() {
        let mut state = model(1, 1);
        let message = state.open_message(1).unwrap();
        let room = state.acquire_room().unwrap();
        assert_eq!(state.room_wait_reason(), WaitClass::CallerMustReleaseRooms);
        state.commit(message, room, 128).unwrap();
        assert_eq!(state.room_wait_reason(), WaitClass::WorkerCanProgress);
        state.worker_finish_room(room).unwrap();
        assert_eq!(state.room_wait_reason(), WaitClass::Available);
        state.check_invariants().unwrap();
    }

    #[test]
    fn room_can_be_acquired_before_routing_to_a_message() {
        let mut state = model(2, 1);
        let a = state.open_message(1).unwrap();
        let b = state.open_message(2).unwrap();
        let room = state.acquire_room().unwrap();
        state.commit(b, room, 64).unwrap();
        state.end_message(a).unwrap();
        state.end_message(b).unwrap();
        state.worker_finish_room(room).unwrap();
        assert_eq!(state.pull_event().unwrap().unwrap().tag, 1);
        assert_eq!(state.pull_event().unwrap().unwrap().tag, 2);
        state.check_invariants().unwrap();
    }

    #[test]
    fn end_waits_for_committed_rooms_but_workers_can_free_the_room() {
        let mut state = model(1, 1);
        let message = state.open_message(1).unwrap();
        let room = state.acquire_room().unwrap();
        state.commit(message, room, ROOM_LEN).unwrap();
        state.end_message(message).unwrap();
        assert_eq!(state.message_wait_reason(), WaitClass::WorkerCanProgress);
        state.worker_finish_room(room).unwrap();
        assert_eq!(state.pull_event().unwrap().unwrap().kind, EventKind::Hashed);
        state.check_invariants().unwrap();
    }

    #[test]
    fn dropping_message_is_observed_by_the_next_api_call() {
        let mut state = model(1, 1);
        let message = state.open_message(4).unwrap();
        state.drop_message(message).unwrap();
        assert!(state.messages[&message].drop_pending);
        let next = state.open_message(5).unwrap();
        assert!(!state.messages.contains_key(&message));
        assert_eq!(state.events.front().unwrap().kind, EventKind::Aborted);
        assert_eq!(state.messages[&next].tag, 5);
        state.check_invariants().unwrap();
    }

    #[test]
    fn dropped_message_waits_for_already_committed_room_work() {
        let mut state = model(1, 1);
        let message = state.open_message(1).unwrap();
        let room = state.acquire_room().unwrap();
        state.commit(message, room, 128).unwrap();
        state.drop_message(message).unwrap();
        let _ = state.room_wait_class(); // A public-call boundary observes the drop in the clone only.
        state.pull_event().unwrap();
        assert_eq!(
            state.rooms[room],
            RoomState::InFlight {
                message_id: message
            }
        );
        state.worker_finish_room(room).unwrap();
        assert_eq!(state.events.front().unwrap().kind, EventKind::Aborted);
        state.check_invariants().unwrap();
    }

    #[test]
    fn room_wait_call_observes_dropped_message_before_blocking() {
        let mut state = model(1, 1);
        let message = state.open_message(41).unwrap();
        state.drop_message(message).unwrap();
        let held_room = state.acquire_room().unwrap();
        assert!(state.acquire_room().is_err());
        assert!(!state.messages.contains_key(&message));
        assert_eq!(
            state.events.front().map(|event| event.kind),
            Some(EventKind::Aborted)
        );
        state.drop_room(held_room).unwrap();
        assert_eq!(state.acquire_room().unwrap(), held_room);
        state.check_invariants().unwrap();
    }

    #[test]
    fn message_wait_call_keeps_drop_sweep_when_event_pool_is_full() {
        let mut state = model(1, 1);
        let first = state.open_message(1).unwrap();
        state.end_message(first).unwrap();
        let dropped = state.open_message(2).unwrap();
        state.drop_message(dropped).unwrap();
        assert!(state.open_message(3).is_err());
        assert!(!state.messages[&dropped].drop_pending);
        assert_eq!(
            state.messages[&dropped]
                .finished_pending
                .as_ref()
                .map(|event| event.kind),
            Some(EventKind::Aborted)
        );
        assert_eq!(
            state.message_wait_reason(),
            WaitClass::ConsumerMustPullEvents
        );
        state.check_invariants().unwrap();
    }

    #[test]
    fn lent_guard_drop_cancels_unstarted_work_and_waits_for_running_reader() {
        let mut state = model(1, 1);
        let message = state.open_message(1).unwrap();
        state.lent(message).unwrap();
        state.worker_start_direct(message).unwrap();
        state.drop_lent_guard(message).unwrap();
        assert_eq!(state.messages[&message].direct_pending, 0);
        assert_eq!(state.messages[&message].direct_running, 1);
        assert!(state.events.is_empty());
        state.worker_finish_direct(message).unwrap();
        let event = state.pull_event().unwrap().unwrap();
        assert_eq!(event.kind, EventKind::Aborted);
        assert_eq!(event.owner_token, None);
        assert_eq!(event.batch_token, None);
        state.check_invariants().unwrap();
    }

    #[test]
    fn limited_lent_guard_can_be_dropped_after_event_materialization_before_pull() {
        let mut state = model(1, 1);
        let message = state.open_message(1).unwrap();
        state.lent(message).unwrap();
        state.worker_start_direct(message).unwrap();
        state.worker_start_direct(message).unwrap();
        state.worker_finish_direct(message).unwrap();
        state.worker_finish_direct(message).unwrap();
        assert_eq!(state.events.front().unwrap().kind, EventKind::Hashed);
        state.drop_lent_guard(message).unwrap();
        assert_eq!(state.pull_event().unwrap().unwrap().kind, EventKind::Hashed);
        state.check_invariants().unwrap();
    }

    #[test]
    fn limited_lent_guard_can_be_dropped_while_materialized_event_is_deferred() {
        let mut state = model(1, 1);
        let blocker = state.open_message(10).unwrap();
        state.end_message(blocker).unwrap();

        let message = state.open_message(11).unwrap();
        state.lent(message).unwrap();
        state.worker_start_direct(message).unwrap();
        state.worker_start_direct(message).unwrap();
        state.worker_finish_direct(message).unwrap();
        state.worker_finish_direct(message).unwrap();

        assert_eq!(
            state.messages[&message]
                .finished_pending
                .as_ref()
                .map(|event| event.kind),
            Some(EventKind::Hashed)
        );
        assert_eq!(
            state.lent_guards.get(&message),
            Some(&LentGuardState::EventMaterialized)
        );

        state.drop_lent_guard(message).unwrap();
        assert!(!state.lent_guards.contains_key(&message));

        assert_eq!(state.pull_event().unwrap().unwrap().message_id, blocker);
        let event = state.pull_event().unwrap().unwrap();
        assert_eq!(event.message_id, message);
        assert_eq!(event.kind, EventKind::Hashed);
        state.check_invariants().unwrap();
    }

    #[test]
    fn lent_guard_can_be_dropped_after_event_delivery() {
        let mut state = model(1, 1);
        let message = state.open_message(1).unwrap();
        state.lent(message).unwrap();
        state.worker_start_direct(message).unwrap();
        state.worker_start_direct(message).unwrap();
        state.worker_finish_direct(message).unwrap();
        state.worker_finish_direct(message).unwrap();
        let event = state.pull_event().unwrap().unwrap();
        assert_eq!(event.kind, EventKind::Hashed);
        assert_eq!(
            state.lent_guards.get(&message),
            Some(&LentGuardState::EventDelivered)
        );
        state.drop_lent_guard(message).unwrap();
        state.check_invariants().unwrap();
    }

    #[test]
    fn successful_lent_event_does_not_carry_the_guard() {
        let mut state = model(1, 1);
        let message = state.open_message(1).unwrap();
        state.lent(message).unwrap();
        state.worker_start_direct(message).unwrap();
        state.worker_start_direct(message).unwrap();
        state.worker_finish_direct(message).unwrap();
        state.worker_finish_direct(message).unwrap();
        let event = state.events.front().unwrap();
        assert_eq!(event.kind, EventKind::Hashed);
        assert_eq!(event.owner_token, None);
        assert_eq!(event.batch_token, None);
        assert_eq!(
            state.lent_guards.get(&message),
            Some(&LentGuardState::EventMaterialized)
        );
    }

    #[test]
    fn batch_returns_both_owned_storage_tokens() {
        let mut state = model(1, 1);
        let message = state.open_message(8).unwrap();
        state.batch(message, 10, 20).unwrap();
        state.worker_start_direct(message).unwrap();
        state.worker_finish_direct(message).unwrap();
        let event = state.pull_event().unwrap().unwrap();
        assert_eq!(event.kind, EventKind::Batch);
        assert_eq!(event.batch_token, Some((10, 20)));
        state.check_invariants().unwrap();
    }

    #[test]
    fn mixed_input_paths_are_rejected() {
        let mut state = model(1, 1);
        let message = state.open_message(1).unwrap();
        let room = state.acquire_room().unwrap();
        state.commit(message, room, 1).unwrap();
        assert!(state.owned(message, 9).is_err());
        state.check_invariants().unwrap();
    }

    #[test]
    fn foreign_message_token_is_rejected_by_the_other_pipeline() {
        let mut owner = model(1, 1);
        let mut other = model(1, 1);
        let token = owner.open_message_token(1).unwrap();
        assert!(other.end_token(token).is_err());
        assert_eq!(owner.messages.len(), 1);
        owner.check_invariants().unwrap();
    }

    #[test]
    fn foreign_room_token_is_rejected_by_the_other_pipeline() {
        let mut owner = model(1, 1);
        let mut other = model(1, 1);
        let room = owner.acquire_room_token().unwrap();
        let mut message = other.open_message_token(2).unwrap();
        assert!(other.commit_tokens(&mut message, room, 1).is_err());
        assert_eq!(owner.rooms[0], RoomState::Held);
        assert_eq!(other.messages[&message.message_id()].path, InputPath::None);
        owner.check_invariants().unwrap();
        other.check_invariants().unwrap();
    }

    #[test]
    fn own_message_and_room_tokens_commit_and_end() {
        let mut state = model(1, 1);
        let mut message = state.open_message_token(3).unwrap();
        let room = state.acquire_room_token().unwrap();
        state.commit_tokens(&mut message, room, 17).unwrap();
        state.end_token(message).unwrap();
        state.worker_finish_room(0).unwrap();
        assert_eq!(state.pull_event().unwrap().unwrap().kind, EventKind::Hashed);
        state.check_invariants().unwrap();
    }

    #[test]
    fn dropping_message_token_is_observed_on_the_next_pipeline_call() {
        let mut state = model(1, 1);
        let message = state.open_message_token(31).unwrap();
        state.drop_message_token(message).unwrap();
        assert_eq!(state.messages.len(), 1);
        let replacement = state.open_message(32).unwrap();
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[&replacement].tag, 32);
        assert_eq!(
            state.events.front().map(|event| event.kind),
            Some(EventKind::Aborted)
        );
        state.check_invariants().unwrap();
    }

    #[test]
    fn dropping_uncommitted_room_token_returns_room_capacity() {
        let mut state = model(1, 1);
        let room = state.acquire_room_token().unwrap();
        assert_eq!(state.rooms[0], RoomState::Held);
        state.drop_room_token(room).unwrap();
        assert_eq!(state.rooms[0], RoomState::Free);
        state.check_invariants().unwrap();
    }

    #[test]
    fn oversized_commit_is_rejected_without_state_corruption() {
        let mut state = model(1, 1);
        let message = state.open_message(1).unwrap();
        let room = state.acquire_room().unwrap();
        assert!(state.commit(message, room, ROOM_LEN + 1).is_err());
        assert_eq!(state.rooms[room], RoomState::Held);
        assert_eq!(state.messages[&message].path, InputPath::None);
        state.check_invariants().unwrap();
    }

    #[test]
    fn events_follow_completion_order_not_submission_order() {
        let mut state = model(2, 2);
        let a = state.open_message(10).unwrap();
        let b = state.open_message(20).unwrap();
        let room_a = state.acquire_room().unwrap();
        let room_b = state.acquire_room().unwrap();
        state.commit(a, room_a, 16).unwrap();
        state.commit(b, room_b, 16).unwrap();
        state.end_message(a).unwrap();
        state.end_message(b).unwrap();
        state.worker_finish_room(room_b).unwrap();
        state.worker_finish_room(room_a).unwrap();
        assert_eq!(state.pull_event().unwrap().unwrap().tag, 20);
        assert_eq!(state.pull_event().unwrap().unwrap().tag, 10);
        state.check_invariants().unwrap();
    }

    #[test]
    fn a_finished_pending_result_is_not_an_extra_event_slot() {
        let mut state = model(1, 1);
        let first = state.open_message(1).unwrap();
        state.end_message(first).unwrap();
        let second = state.open_message(2).unwrap();
        state.end_message(second).unwrap();
        assert_eq!(state.events.len(), 1);
        assert_eq!(state.messages.len(), 1);
        assert!(state.messages[&second].finished_pending.is_some());
        state.check_invariants().unwrap();
    }
}
