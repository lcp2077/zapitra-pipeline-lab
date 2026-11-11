//! Compile-only consumer probe for the BLAKE3 Pipeline API skeleton.
//! This file is type-checked against upstream signatures; it must not be run.
#![allow(dead_code)]

use pipeline_api_skeleton::{Batch, Event, Limits, Mode, Pipeline, ROOM_LEN};

pub fn make_basic_pipeline() -> Pipeline<u64> {
    Pipeline::new(
        Mode::Plain,
        Limits {
            messages: 2,
            rooms: 2,
        },
    )
}

pub fn make_owner_pipeline() -> Pipeline<u64, Vec<u8>> {
    Pipeline::with_owners(
        Mode::Plain,
        Limits {
            messages: 2,
            rooms: 2,
        },
    )
}

/// Models a streamed input: take a room before assigning it to a message.
pub async fn room_commit_end(pipeline: &mut Pipeline<u64>) {
    let mut message = pipeline.message(1, Some(64)).await;
    let mut room = pipeline.room().await;
    let bytes = room.bytes_mut();
    let written = bytes.len().min(ROOM_LEN).min(64);
    bytes[..written].fill(0xA5);
    let _stable_address_for_external_io = room.as_mut_ptr();
    pipeline.commit(&mut message, room, written);
    pipeline.end(message);
}

/// Models a zero-copy ownership hand-off; the owner comes back in an event.
pub async fn owned_handoff(pipeline: &mut Pipeline<u64, Vec<u8>>) {
    let message = pipeline.message(2, Some(3)).await;
    pipeline.owned(message, vec![0x01, 0x02, 0x03]);
}

/// Models a borrowed input whose guard remains live through event pulling.
/// Dropping this function early drops the guard before the borrowed buffer.
pub async fn lent_guard_lifetime(pipeline: &mut Pipeline<u64>, bytes: &[u8]) {
    let message = pipeline.message(3, Some(bytes.len() as u64)).await;
    // SAFETY: this probe holds the guard and never leaks it. This checks the
    // signature/lifetime shape only; the upstream skeleton has no runtime body.
    let guard = unsafe { pipeline.lent(message, bytes) };
    loop {
        if let Some(event) = pipeline.next_event() {
            match event {
                Event::Hashed {
                    tag: 3,
                    hash,
                    owner,
                } => {
                    drop((hash, owner));
                    break;
                }
                Event::Aborted { tag: 3, owner } => {
                    drop(owner);
                    break;
                }
                other => drop(other),
            }
        } else {
            pipeline.ready().await;
        }
    }
    drop(guard);
}

/// Models owned batch hand-off and retrieval of both storage objects.
pub async fn owned_batch_round_trip(pipeline: &mut Pipeline<u64>) {
    let batch = Batch::new(64, 4);
    let digests = vec![[0u8; 32]; 4].into_boxed_slice();
    let message = pipeline.message(4, None).await;
    pipeline.batch(message, batch, digests);
    pipeline.ready().await;
    if let Some(Event::Batch { batch, digests, .. }) = pipeline.next_event() {
        drop((batch, digests));
    }
}

/// A returned event is handled after the Pipeline call ended, so it can be
/// called again. The probe does not execute this future.
pub async fn pull_events_and_continue(pipeline: &mut Pipeline<u64>) {
    pipeline.ready().await;
    while let Some(event) = pipeline.next_event() {
        match event {
            Event::Hashed { tag, hash, owner } => {
                drop((hash, owner));
                let follow_up = pipeline.message(tag, None).await;
                pipeline.end(follow_up);
            }
            Event::Aborted { tag, owner } => {
                drop(owner);
                let follow_up = pipeline.message(tag, None).await;
                pipeline.abort(follow_up);
            }
            Event::Batch {
                tag,
                batch,
                digests,
            } => {
                drop((batch, digests));
                let follow_up = pipeline.message(tag, None).await;
                pipeline.end(follow_up);
            }
        }
    }
}

// Cargo example targets are binaries; this empty entry point is not a runtime test.
fn main() {}
