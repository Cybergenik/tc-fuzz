use std::collections::hash_map::DefaultHasher;
use std::ffi::CString;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::os::raw::c_char;
use libafl::corpus::{InMemoryCorpus, OnDiskCorpus};
use libafl::executors::{ExitKind, InProcessExecutor};
use libafl::feedbacks::{CrashFeedback, MaxMapFeedback, TimeoutFeedback};
use libafl::fuzzer::{Fuzzer, StdFuzzer};
use libafl::inputs::{BytesInput, HasMutatorBytes};
use libafl::mutators::{havoc_mutations, HavocScheduledMutator, SingleChoiceScheduledMutator};
use libafl::observers::{CanTrack, HitcountsMapObserver, StdMapObserver};
use libafl::schedulers::{CoverageAccountingScheduler, QueueScheduler};
use libafl::stages::StdMutationalStage;
use libafl::state::StdState;
use libafl::Error;
use libafl::{events, feedback_or};
use libafl_bolts::rands::StdRand;
use libafl_bolts::tuples::tuple_list;

use crate::generator::ExprGenerator;
use crate::mutator::ExprMutator;
use crate::oracle::PythonOracle;
use crate::FuzzerState;

unsafe extern "C" {
    fn calc_eval(expr: *mut c_char) -> f64;
}

const DIFF_THRESHOLD: f64 = 1e-6;

pub fn run_fuzzer<EM>(
    state: Option<FuzzerState>,
    mut mgr: EM,
    desc: events::launcher::ClientDescription,
    verbose: bool,
) -> Result<(), Error>
where
    EM: events::EventFirer<BytesInput, FuzzerState>
        + events::EventReceiver<BytesInput, FuzzerState>
        + events::EventRestarter<FuzzerState>
        + events::ProgressReporter<FuzzerState>
        + events::SendExiting,
{
    let map_size: usize = if cfg!(feature = "ubsan") { 512 } else { 256 };
    let shmem_map = vec![0u8; map_size].leak().as_mut_ptr();
    unsafe { libafl_targets::EDGES_MAP_PTR = shmem_map; }

    let edges_observer = unsafe {
        HitcountsMapObserver::new(StdMapObserver::from_mut_ptr("edges", shmem_map, map_size))
            .track_indices()
    };

    let mut feedback = MaxMapFeedback::new(&edges_observer);
    let mut objective = feedback_or!(CrashFeedback::new(), TimeoutFeedback::new());

    let is_fresh = state.is_none();
    let mut state = state.unwrap_or_else(|| {
        StdState::new(
            StdRand::with_seed((1337 * (desc.id() as u64 + 13)) ^ libafl_bolts::current_nanos()),
            InMemoryCorpus::<BytesInput>::new(),
            OnDiskCorpus::new("./crashes").expect("crashes dir"),
            &mut feedback,
            &mut objective,
        )
        .expect("state")
    });

    let scheduler = CoverageAccountingScheduler::new(
        &edges_observer,
        &mut state,
        QueueScheduler::new(),
        vec![0u32; map_size].leak(),
    );
    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut log_file = if verbose {
        Some(std::fs::File::create(format!("./fuzz_log_{}.txt", desc.id())).expect("open log"))
    } else {
        None
    };

    let mut python = PythonOracle::new().map_err(|e| Error::unknown(e))?;

    let mut harness = |input: &BytesInput| {
        let bytes = input.mutator_bytes();
        if bytes.is_empty() {
            return ExitKind::Ok;
        }

        let filtered: Vec<u8> = bytes.iter().copied().filter(|&b| b != 0).collect();
        if filtered.is_empty() {
            return ExitKind::Ok;
        }

        let c_str = CString::new(filtered).unwrap();
        let expr_str = match c_str.to_str() {
            Ok(s) if !s.is_empty() => s,
            _ => return ExitKind::Ok,
        };

        let py_result = python.eval(expr_str);
        let tc_result = unsafe { calc_eval(c_str.as_ptr() as *mut c_char) };

        let diff_msg = match (tc_result.is_nan(), py_result) {
            // both fail
            (true, Err(_)) => {
                if let Some(f) = &mut log_file {
                    let _ = writeln!(f, "{expr_str} => both error");
                }
                None
            }
            // tc-calc error, python got a value — diff.
            (true, Ok(py)) => Some(format!("{expr_str}\n[tc-calc]: NaN  !=  [PYTHON3]: {py}")),
            // tc-calc got a value, python errored — diff.
            (false, Err(_)) => Some(format!(
                "{expr_str}\n[tc-calc]: {tc_result}  !=  [PYTHON3]: ERR"
            )),
            // both valid
            (false, Ok(expected)) => {
                // same - log if configured 
                if tc_result == expected {
                    if let Some(f) = &mut log_file {
                        let _ = writeln!(f, "{expr_str} => {tc_result}");
                    }
                    None
                // both finite and diff
                } else if tc_result.is_finite() && expected.is_finite() {
                    let diff = (tc_result - expected).abs();
                    let magnitude = tc_result.abs().max(expected.abs()).max(1.0);
                    if diff > magnitude * DIFF_THRESHOLD {
                        Some(format!(
                            "{expr_str}\n[tc-calc]: {tc_result}  !=  [PYTHON3]: {expected}"
                        ))
                    } else {
                        if let Some(f) = &mut log_file {
                            let _ = writeln!(f, "{expr_str} => {tc_result}");
                        }
                        None
                    }
                // One fin/inf/nan
                } else {
                    Some(format!(
                        "{expr_str}\n[tc-calc]: {tc_result}  !=  [PYTHON3]: {expected}"
                    ))
                }
            }
        };

        if let Some(msg) = diff_msg {
            // hash on coverage to reduce diffs clutte
            let cov = unsafe { std::slice::from_raw_parts(shmem_map, map_size) };
            let mut hasher = DefaultHasher::new();
            cov.hash(&mut hasher);
            let path = format!("./diffs/{:016x}", hasher.finish());
            let should_write = match std::fs::metadata(&path) {
                Ok(meta) => msg.len() < meta.len() as usize,
                Err(_) => true,
            };
            if should_write {
                let _ = std::fs::write(&path, &msg);
            }
        }

        ExitKind::Ok
    };

    let mut executor = InProcessExecutor::new(
        &mut harness,
        tuple_list!(edges_observer),
        &mut fuzzer,
        &mut state,
        &mut mgr,
    )
    .expect("executor");

    if is_fresh {
        state
            .generate_initial_inputs(
                &mut fuzzer,
                &mut executor,
                &mut ExprGenerator::new(5),
                &mut mgr,
                256,
            )
            .expect("initial inputs");
    }

    let mut stages = tuple_list!(
        StdMutationalStage::new(SingleChoiceScheduledMutator::new(tuple_list!(
            ExprMutator::new()
        ))),
        StdMutationalStage::new(HavocScheduledMutator::with_max_stack_pow(
            havoc_mutations(),
            2,
        )),
    );

    fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)?;
    Ok(())
}
