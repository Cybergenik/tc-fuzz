mod fuzzer;
mod generator;
mod mutator;
mod oracle;

use libafl::corpus::{InMemoryCorpus, OnDiskCorpus};
use libafl::events::launcher::Launcher;
use libafl::events::EventConfig;
use libafl::inputs::BytesInput;
use libafl::monitors::tui::TuiMonitor;
use libafl::state::StdState;
use libafl::Error;
use libafl_bolts::core_affinity::Cores;
use libafl_bolts::rands::StdRand;
use libafl_bolts::shmem::{ShMemProvider, StdShMemProvider};

type FuzzerState =
    StdState<InMemoryCorpus<BytesInput>, BytesInput, StdRand, OnDiskCorpus<BytesInput>>;

fn main() {
    std::fs::create_dir_all("./crashes").expect("create crashes dir");
    std::fs::create_dir_all("./diffs").expect("create diffs dir");

    let args: Vec<String> = std::env::args().collect();
    let cores_str = if let Some(pos) = args.iter().position(|a| a == "--cores") {
        args.get(pos + 1).map(|s| s.as_str()).unwrap_or("0")
    } else {
        args.get(1).map(|s| s.as_str()).unwrap_or("0")
    };
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    let cores = Cores::from_cmdline(cores_str)
        .expect("invalid cores spec (use: all, 0-3, 0,1,2, etc.)");

    let monitor = TuiMonitor::builder()
        .title("TC-calc Fuzz")
        .enhanced_graphics(true)
        .build();

    let shmem_provider = StdShMemProvider::new().expect("shmem provider");

    let result = Launcher::builder()
        .shmem_provider(shmem_provider)
        .monitor(monitor)
        .cores(&cores)
        .configuration(EventConfig::from_name("tc-calc-fuzzer"))
        .run_client(move |state: Option<FuzzerState>, mgr, desc| {
            fuzzer::run_fuzzer(state, mgr, desc, verbose)
        })
        .build()
        .launch();

    match result {
        Ok(()) | Err(Error::ShuttingDown) => {}
        Err(e) => eprintln!("[ERROR] {e}"),
    }
}
