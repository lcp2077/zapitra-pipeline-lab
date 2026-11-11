use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use zapitra::{
    EventKind, ExploreConfig, InputPath, LentGuardState, Limits, RoomState, State, WaitClass,
    explore, render_json, render_text,
};

const VERSION: &str = "0.1";

macro_rules! model_try {
    ($expression:expr) => {
        $expression.map_err(std::io::Error::other)?
    };
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ZAPITRA: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    let options: Vec<String> = args.collect();
    match command.as_str() {
        "help" | "--help" | "-h" => print_help(),
        "version" | "--version" | "-V" => println!("ZAPITRA {VERSION}"),
        "doctor" => doctor(),
        "selfcheck" => selfcheck(parse_config(&options)?)?,
        "explore" => explore_command(parse_config(&options)?)?,
        "upstream-probes" => upstream_probes(),
        other => {
            return Err(std::io::Error::other(format!(
                "unknown command: {other}. Run `zapitra help`."
            ))
            .into());
        }
    }
    Ok(())
}

fn print_help() {
    println!(
        "ZAPITRA {VERSION} — Pipeline API contract and state-space laboratory\n\n\
         COMMANDS:\n\
           zapitra doctor          Read-only environment diagnostics\n\
           zapitra selfcheck       Run fixed contract scenarios and bounded exploration\n\
           zapitra explore         Explore states with worker-only wait oracles\n\
           zapitra upstream-probes Show how to compile upstream doctest claims\n\n\
         OPTIONS for selfcheck/explore:\n\
           --messages N     message-record and event-pool capacity (default 2)\n\
           --rooms N        room-pool capacity (default 2)\n\
           --depth N        BFS maximum depth (default 8)\n\
           --states N       maximum distinct states (default 12000)\n\
           --report PATH    write a JSON report\n\n\
         Scope: a bounded Rust reference model. The upstream skeleton has todo!()\
         bodies. ZAPITRA does not claim runtime hashing, performance, zero-allocation,\
         or memory-safety validation."
    );
}

#[derive(Clone)]
struct CliConfig {
    explore: ExploreConfig,
    report: Option<PathBuf>,
}

fn parse_config(args: &[String]) -> Result<CliConfig, Box<dyn Error>> {
    let mut config = ExploreConfig::default();
    let mut report = None;
    let mut index = 0;
    while index < args.len() {
        let name = &args[index];
        let value = args
            .get(index + 1)
            .ok_or_else(|| std::io::Error::other(format!("missing value for option {name}")))?;
        match name.as_str() {
            "--messages" => config.limits.messages = value.parse()?,
            "--rooms" => config.limits.rooms = value.parse()?,
            "--depth" => config.max_depth = value.parse()?,
            "--states" => config.max_states = value.parse()?,
            "--report" => report = Some(PathBuf::from(value)),
            _ => return Err(std::io::Error::other(format!("unknown option: {name}")).into()),
        }
        index += 2;
    }
    config.limits.validate().map_err(std::io::Error::other)?;
    if config.max_depth == 0 || config.max_states == 0 {
        return Err(std::io::Error::other("--depth and --states must be greater than zero").into());
    }
    Ok(CliConfig {
        explore: config,
        report,
    })
}

fn doctor() {
    println!("ZAPITRA {VERSION} — diagnostics (read-only)");
    println!("OS: {}", env::consts::OS);
    println!("Architecture: {}", env::consts::ARCH);
    println!(
        "Logical CPUs: {}",
        std::thread::available_parallelism().map_or(1, |count| count.get())
    );
    println!("Rust: {}", command_version("rustc", &["--version"]));
    println!("Cargo: {}", command_version("cargo", &["--version"]));
    println!("Git: {}", command_version("git", &["--version"]));
    println!("GitHub CLI: {}", command_version("gh", &["--version"]));
    println!("Upstream status: design skeleton; runtime bodies remain todo!()");
    println!("No network, Wi-Fi, firewall, or system configuration was changed.");
}

fn command_version(program: &str, args: &[&str]) -> String {
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("available")
            .to_owned(),
        _ => "not found or unavailable".into(),
    }
}

fn selfcheck(config: CliConfig) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let checks = fixed_checks()?;
    let report = model_try!(explore(config.explore.clone()));
    if report.state_limit_hit {
        return Err(std::io::Error::other(
            "bounded exploration reached --states; increase the state budget before publication",
        )
        .into());
    }
    let json = format!(
        "{{\n  \"schema\": \"zapitra.selfcheck.v3\",\n  \"project\": \"ZAPITRA\",\n  \"version\": \"{VERSION}\",\n  \"fixed_checks_passed\": {},\n  \"fixed_checks_total\": {},\n  \"elapsed_ms\": {},\n  \"exploration\": {},\n  \"scope\": \"reference-model and bounded state-space checks only\"\n}}\n",
        checks.len(),
        checks.len(),
        started.elapsed().as_millis(),
        trim_json_object(&render_json(&report, &config.explore))?
    );
    validate_json_shape(&json)?;
    if let Some(path) = &config.report {
        write_text(path, &json)?;
        println!("Self-check report: {}", path.display());
    }
    println!(
        "ZAPITRA fixed contract checks: {}/{} PASS",
        checks.len(),
        checks.len()
    );
    for (name, detail) in checks {
        println!("  PASS {name} — {detail}");
    }
    println!("{}", render_text(&report, &config.explore));
    Ok(())
}

fn fixed_checks() -> Result<Vec<(&'static str, &'static str)>, Box<dyn Error>> {
    let mut checks = Vec::new();

    // Caller tags are payload; model IDs remain unique.
    let mut duplicate_tags = model_try!(State::new(Limits {
        messages: 2,
        rooms: 1
    }));
    let a = model_try!(duplicate_tags.open_message(9));
    let b = model_try!(duplicate_tags.open_message(9));
    if a == b {
        return Err(
            std::io::Error::other("duplicate tags collapsed to one internal message ID").into(),
        );
    }
    model_try!(duplicate_tags.end_message(a));
    model_try!(duplicate_tags.end_message(b));
    model_try!(duplicate_tags.check_invariants());
    checks.push((
        "duplicate_tags",
        "repeated caller tags retain distinct internal message IDs",
    ));

    // A room can be acquired before its destination message is known.
    let mut room_routing = model_try!(State::new(Limits {
        messages: 2,
        rooms: 1
    }));
    let first = model_try!(room_routing.open_message(1));
    let second = model_try!(room_routing.open_message(2));
    let room = model_try!(room_routing.acquire_room());
    model_try!(room_routing.commit(second, room, 32));
    model_try!(room_routing.end_message(first));
    model_try!(room_routing.end_message(second));
    model_try!(room_routing.worker_finish_room(room));
    model_try!(room_routing.check_invariants());
    checks.push((
        "room_before_routing",
        "room ownership begins only at commit",
    ));

    // The message-record pool and event pool have the same configured size,
    // but are distinct resources. A finished result waits in its record when
    // event slots are full; this is an explicit model assumption.
    let mut capacities = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let first = model_try!(capacities.open_message(3));
    model_try!(capacities.end_message(first));
    let second = model_try!(capacities.open_message(4));
    model_try!(capacities.end_message(second));
    if capacities.message_wait_reason() != WaitClass::ConsumerMustPullEvents {
        return Err(std::io::Error::other(
            "full event pool was not classified as consumer backpressure",
        )
        .into());
    }
    model_try!(capacities.check_invariants());
    checks.push((
        "separate_pools",
        "message records and completion events use separate bounded pools",
    ));

    // A worker can release a message slot only while event storage is available.
    let mut worker_wait = model_try!(State::new(Limits {
        messages: 2,
        rooms: 1
    }));
    let active = model_try!(worker_wait.open_message(5));
    model_try!(worker_wait.owned(active, 50));
    model_try!(worker_wait.worker_start_direct(active));
    let other = model_try!(worker_wait.open_message(6));
    if worker_wait.message_wait_reason() != WaitClass::WorkerCanProgress {
        return Err(std::io::Error::other("worker-backed message wait was misclassified").into());
    }
    model_try!(worker_wait.end_message(other));
    model_try!(worker_wait.worker_finish_direct(active));
    model_try!(worker_wait.check_invariants());
    checks.push((
        "worker_progress",
        "worker-only reachability identifies a slot that can be freed",
    ));

    // A full room pool containing only uncommitted caller-held rooms cannot be
    // made available by a worker.
    let mut caller_room = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let _room = model_try!(caller_room.acquire_room());
    if caller_room.room_wait_reason() != WaitClass::CallerMustReleaseRooms {
        return Err(std::io::Error::other("caller-held room wait was misclassified").into());
    }
    model_try!(caller_room.check_invariants());
    checks.push((
        "caller_held_room",
        "uncommitted room capacity requires caller action",
    ));

    // An early Lent guard drop cancels unstarted model work and waits for the
    // already-running abstract reader to return.
    let mut lent = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let message = model_try!(lent.open_message(7));
    model_try!(lent.lent(message));
    model_try!(lent.worker_start_direct(message));
    model_try!(lent.drop_lent_guard(message));
    model_try!(lent.worker_finish_direct(message));
    let event = model_try!(lent.pull_event())
        .ok_or_else(|| std::io::Error::other("missing Lent cancellation event"))?;
    if event.kind != EventKind::Aborted {
        return Err(std::io::Error::other("early Lent guard drop did not produce Aborted").into());
    }
    checks.push((
        "lent_cancel",
        "early guard drop drains active readers before Aborted",
    ));

    // Limited-Lent policy: the guard protects the borrowed bytes until all
    // abstract readers finish and the terminal event is materialized. The
    // consumer may pull that event later; delivery is not the memory boundary.
    let mut lent_done = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let message = model_try!(lent_done.open_message(8));
    model_try!(lent_done.lent(message));
    model_try!(lent_done.worker_start_direct(message));
    model_try!(lent_done.worker_start_direct(message));
    model_try!(lent_done.worker_finish_direct(message));
    model_try!(lent_done.worker_finish_direct(message));
    if lent_done.lent_guards.get(&message) != Some(&LentGuardState::EventMaterialized) {
        return Err(
            std::io::Error::other("event-materialized Lent guard state was not retained").into(),
        );
    }
    model_try!(lent_done.drop_lent_guard(message));
    if model_try!(lent_done.pull_event()).is_none() {
        return Err(
            std::io::Error::other("materialized Lent event was lost after guard release").into(),
        );
    }
    checks.push((
        "lent_completion_boundary",
        "guard released after event materialization does not retroactively abort",
    ));

    // Option 2, limited Lent: a full event queue may defer delivery, but all
    // readers are done once the Hashed result is materialized. The guard can
    // then be released while the event waits in the message record.
    let mut lent_pending = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let blocker = model_try!(lent_pending.open_message(80));
    model_try!(lent_pending.end_message(blocker));
    let lent_message = model_try!(lent_pending.open_message(81));
    model_try!(lent_pending.lent(lent_message));
    model_try!(lent_pending.worker_start_direct(lent_message));
    model_try!(lent_pending.worker_start_direct(lent_message));
    model_try!(lent_pending.worker_finish_direct(lent_message));
    model_try!(lent_pending.worker_finish_direct(lent_message));
    if lent_pending
        .messages
        .get(&lent_message)
        .and_then(|record| record.finished_pending.as_ref())
        .map(|event| event.kind)
        != Some(EventKind::Hashed)
    {
        return Err(std::io::Error::other(
            "materialized Lent event was not retained behind a full event queue",
        )
        .into());
    }
    model_try!(lent_pending.drop_lent_guard(lent_message));
    let first_event = model_try!(lent_pending.pull_event())
        .ok_or_else(|| std::io::Error::other("missing event-queue blocker"))?;
    if first_event.message_id == lent_message {
        return Err(
            std::io::Error::other("pending Lent event bypassed the older completion").into(),
        );
    }
    let lent_event = model_try!(lent_pending.pull_event())
        .ok_or_else(|| std::io::Error::other("missing deferred Lent event"))?;
    if lent_event.message_id != lent_message || lent_event.kind != EventKind::Hashed {
        return Err(std::io::Error::other(
            "limited Lent changed a materialized success while awaiting delivery",
        )
        .into());
    }
    model_try!(lent_pending.check_invariants());
    checks.push((
        "lent_limited_pending_event",
        "guard can be released after Hashed materialization while event delivery is backpressured",
    ));

    let mut batch = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let message = model_try!(batch.open_message(10));
    model_try!(batch.batch(message, 100, 101));
    model_try!(batch.worker_start_direct(message));
    model_try!(batch.worker_finish_direct(message));
    let event = model_try!(batch.pull_event())
        .ok_or_else(|| std::io::Error::other("missing Batch event"))?;
    if event.kind != EventKind::Batch || event.batch_token != Some((100, 101)) {
        return Err(
            std::io::Error::other("Batch and digest storage were not returned together").into(),
        );
    }
    checks.push(("batch_storage", "Batch and digest storage return together"));

    // Token wrappers exercise the pipeline-identity check through operations.
    let mut token_owner = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let mut token_other = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let foreign_message = model_try!(token_owner.open_message_token(11));
    if token_other.end_token(foreign_message).is_ok() {
        return Err(std::io::Error::other("foreign Message token was accepted").into());
    }
    let foreign_room = model_try!(token_owner.acquire_room_token());
    let mut local_message = model_try!(token_other.open_message_token(12));
    if token_other
        .commit_tokens(&mut local_message, foreign_room, 1)
        .is_ok()
    {
        return Err(std::io::Error::other("foreign Room token was accepted").into());
    }
    model_try!(token_owner.check_invariants());
    model_try!(token_other.check_invariants());
    checks.push((
        "pipeline_identity",
        "Message and Room token provenance is checked by model operations",
    ));

    let mut dropped = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let message = model_try!(dropped.open_message(13));
    model_try!(dropped.drop_message(message));
    let replacement = model_try!(dropped.open_message(14));
    if dropped.messages.contains_key(&message) || !dropped.messages.contains_key(&replacement) {
        return Err(
            std::io::Error::other("dropped Message was not swept at the next API call").into(),
        );
    }
    if dropped
        .events
        .front()
        .is_none_or(|event| event.kind != EventKind::Aborted)
    {
        return Err(std::io::Error::other("dropped Message did not produce Aborted").into());
    }
    model_try!(dropped.check_invariants());
    checks.push((
        "dropped_message",
        "dropped Message is observed at the next Pipeline call",
    ));

    let mut unused_path_check = model_try!(State::new(Limits {
        messages: 1,
        rooms: 1
    }));
    let message = model_try!(unused_path_check.open_message(15));
    if unused_path_check
        .messages
        .get(&message)
        .map(|record| record.path)
        != Some(InputPath::None)
    {
        return Err(
            std::io::Error::other("new message did not begin without an input path").into(),
        );
    }
    let room = model_try!(unused_path_check.acquire_room());
    if unused_path_check.rooms[room] != RoomState::Held {
        return Err(std::io::Error::other("acquired room was not held").into());
    }
    checks.push((
        "room_state",
        "uncommitted rooms remain independently held by the caller",
    ));

    Ok(checks)
}

fn explore_command(config: CliConfig) -> Result<(), Box<dyn Error>> {
    let report = model_try!(explore(config.explore.clone()));
    let text = render_text(&report, &config.explore);
    print!("{text}");
    if let Some(path) = config.report {
        let json = render_json(&report, &config.explore);
        validate_json_shape(&json)?;
        write_text(&path, &json)?;
        println!("JSON report: {}", path.display());
    }
    if report.state_limit_hit {
        return Err(std::io::Error::other(
            "state budget reached; increase --states before treating the exploration as complete",
        )
        .into());
    }
    Ok(())
}

fn trim_json_object(value: &str) -> Result<String, Box<dyn Error>> {
    let value = value.trim();
    if !value.starts_with('{') || !value.ends_with('}') {
        return Err(std::io::Error::other("explorer did not produce a JSON object").into());
    }
    Ok(value.to_owned())
}

fn validate_json_shape(value: &str) -> Result<(), Box<dyn Error>> {
    let mut depth = 0i64;
    let mut in_string = false;
    let mut escaped = false;
    for character in value.chars() {
        if in_string {
            match character {
                '\\' if !escaped => escaped = true,
                '"' if !escaped => in_string = false,
                _ => escaped = false,
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return Err(std::io::Error::other("report JSON delimiters are unbalanced").into());
        }
    }
    if depth != 0 || in_string {
        return Err(std::io::Error::other("report JSON is incomplete").into());
    }
    Ok(())
}

fn write_text(path: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

fn upstream_probes() {
    println!("BLAKE3-Servil upstream design probes:");
    println!("  repository: https://github.com/servil-software-factory/BLAKE3");
    println!("  branch:     design/one-stream-api");
    println!("  manifest:   docs/experiments/pipeline-api-skeleton/Cargo.toml");
    println!("  command:    ./scripts/check-upstream-skeleton.sh");
    println!("The skeleton has todo!() bodies. Its doctests validate compiler claims only.");
}
