mod cli;

use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use neurogate_limit_watch::{self as ng, VERSION};

use cli::args::{FailOn, parse_args};
use cli::monitor::run_monitor;
use cli::notify::Notifier;
use cli::output::run_once;

fn main() {
    let code = match real_main() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("nglimit: {message}");
            2
        }
    };
    pause_before_exit_if_own_console();
    std::process::exit(code);
}

#[cfg(windows)]
fn pause_before_exit_if_own_console() {
    if windows_console_process_count() <= 1 {
        eprintln!();
        eprint!("Press Enter to exit...");
        let _ = io::stderr().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
}

#[cfg(not(windows))]
fn pause_before_exit_if_own_console() {}

#[cfg(windows)]
fn windows_console_process_count() -> u32 {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleProcessList(process_list: *mut u32, process_count: u32) -> u32;
    }

    let mut processes = [0_u32; 8];
    unsafe { GetConsoleProcessList(processes.as_mut_ptr(), processes.len() as u32) }
}

fn real_main() -> Result<i32, String> {
    let args = parse_args(env::args().skip(1))?;
    if args.help {
        cli::args::print_help();
        return Ok(0);
    }
    if args.version {
        println!("nglimit {VERSION}");
        return Ok(0);
    }
    let mut notifier = Notifier::new(args.notify);
    let http = ng::HttpClient::new(ng::USER_AGENT)?;
    if args.monitor {
        return run_monitor(&args, &mut notifier);
    }

    loop {
        let dotenv = load_config(&args)?;
        let code = run_once(&args, &dotenv, &mut notifier, &http)?;
        if args.watch == 0 {
            return Ok(code);
        }
        if args.fail_on != FailOn::Never && code != 0 {
            return Ok(code);
        }
        thread::sleep(Duration::from_secs(args.watch));
    }
}

fn load_config(args: &cli::args::Args) -> Result<ng::RuntimeConfig, String> {
    ng::RuntimeConfig::from_dotenv(
        args.api_base.clone(),
        &args.api_key_env,
        args.env_file.as_ref(),
    )
}
