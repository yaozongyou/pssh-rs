use ansi_term::Colour::{Green, Red};
use anyhow::Context;
use args::HostInfo;
use rayon::prelude::*;
use ssh2::Session;
use std::fs::metadata;
use std::fs::File;
use std::io::prelude::*;
use std::io::Write;
use std::net::TcpStream;
use std::os::unix::prelude::PermissionsExt;
use std::path::Path;
use std::sync::mpsc::sync_channel;
use std::time::Duration;
use structopt::StructOpt;

mod args;

const DEFAULT_CONFIG_FPATH: &str = "./hosts.toml";
const DEFAULT_CONFIG_TMPL: &str = r#"username = "root"
password = "123456"
port = 22
timeout_ms = 10000
hosts = [
    "192.168.56.101",
    "192.168.56.102"
]

[nginx]
username = "root"
password = "123456"
port = 22
timeout_ms = 10000
hosts = [
    "192.168.57.101",
    "192.168.57.102"
]          
"#;

fn main() -> anyhow::Result<()> {
    let args = args::CommandLineArgs::from_args();

    if let args::Command::Init = args.command {
        if std::fs::exists(DEFAULT_CONFIG_FPATH)? {
            println!("Save the following contents to a file (default to {}) and then", DEFAULT_CONFIG_FPATH);
            println!("using --config {} to use this config file.", DEFAULT_CONFIG_FPATH);
            println!();
            println!("{}", DEFAULT_CONFIG_TMPL);
        } else {
            std::fs::write(DEFAULT_CONFIG_FPATH, DEFAULT_CONFIG_TMPL)?;
            println!("default config template has been written to {}", DEFAULT_CONFIG_FPATH);
            println!("modify this file and using --config {} to use this config file", DEFAULT_CONFIG_FPATH)
        }
        return Ok(());
    }

    let hosts = args.get_hosts()?;
    let (sender, receiver) = sync_channel(hosts.len());

    std::thread::scope(|s| {
        s.spawn({
            let hosts = hosts.clone();
            move || {
                print_main(args.keep_stable, &hosts, receiver);
            }
        });

        rayon::ThreadPoolBuilder::new().num_threads(args.num_threads).build_global().unwrap();
        hosts.par_iter().enumerate().for_each(|(index, host)| {
            let result = match &args.command {
                args::Command::Run { command } => run_command(host, command),
                args::Command::Send { source_fpath, target_fpath } => send_file(host, source_fpath, target_fpath),
                _ => {
                    panic!("shoud not come here")
                }
            };

            sender.send((index, host, result)).unwrap();
        });

        drop(sender);
    });

    Ok(())
}

enum Outcome {
    RunCommandOutcome(RunCommandOutcome),
    SendFileOutcome,
}

struct RunCommandOutcome {
    exit_status: i32,
    out: Vec<u8>,
    err: Vec<u8>,
}

fn run_command(host: &HostInfo, command: &str) -> anyhow::Result<Outcome> {
    let addr = format!("{}:{}", host.host, host.port);
    let tcp = TcpStream::connect_timeout(&addr.parse()?, Duration::from_millis(host.timeout_ms as u64))?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.set_timeout(host.timeout_ms);
    sess.handshake()?;
    sess.set_timeout(0);
    sess.userauth_password(&host.username, &host.password)?;

    let mut channel = sess.channel_session()?;
    let _ = channel.setenv("PSSH_RS_IP", &host.host);
    let _ = channel.setenv("PSSH_RS_PORT", &host.port.to_string());
    channel.exec(command)?;

    let (mut out, mut err) = (vec![], vec![]);

    std::thread::scope(|s| {
        s.spawn(|| {
            if let Err(err) = channel.stream(0).take(1024 * 1024).read_to_end(&mut out) {
                println!("failed to read: {err}");
            }
        });
        s.spawn(|| {
            if let Err(err) = channel.stderr().take(1024 * 1024).read_to_end(&mut err) {
                println!("failed to read: {err}");
            }
        });
    });

    channel.wait_close()?;

    let exit_status = channel.exit_status()?;
    Ok(Outcome::RunCommandOutcome(RunCommandOutcome { exit_status, out, err }))
}

fn send_file(host: &HostInfo, source_fpath: &Path, target_fpath: &Path) -> anyhow::Result<Outcome> {
    let addr = format!("{}:{}", host.host, host.port);
    let tcp = TcpStream::connect_timeout(&addr.parse()?, Duration::from_millis(host.timeout_ms as u64))?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.set_timeout(host.timeout_ms);
    sess.handshake()?;
    sess.set_timeout(0);
    sess.userauth_password(&host.username, &host.password)?;

    let attr = metadata(source_fpath).with_context(|| format!("stat local {}", source_fpath.to_string_lossy()))?;
    let mode = attr.permissions().mode() & 0o777;

    let mut remote_file = sess.scp_send(target_fpath, mode as i32, attr.len(), None)?;

    let mut file = File::open(source_fpath)?;
    std::io::copy(&mut file, &mut remote_file)?;

    // Close the channel and wait for the whole content to be transferred
    remote_file.send_eof()?;
    remote_file.wait_eof()?;
    remote_file.close()?;
    remote_file.wait_close()?;

    Ok(Outcome::SendFileOutcome)
}

fn print_main(
    keep_stable: bool,
    hosts: &[args::HostInfo],
    receiver: std::sync::mpsc::Receiver<(usize, &args::HostInfo, anyhow::Result<Outcome>)>,
) {
    let mut results: Vec<_> = std::iter::repeat_with(|| None).take(hosts.len()).collect();
    let mut print_index: usize = 0;

    loop {
        let Ok((index, host, result)) = receiver.recv() else {
            break;
        };

        if !keep_stable {
            print_outcome(host, &result).unwrap();
            continue;
        }

        results[index] = Some((host, result));
        while print_index < results.len() {
            match results[print_index] {
                Some((host, ref result)) => {
                    print_outcome(host, result).unwrap();
                    print_index += 1;
                }
                None => {
                    break;
                }
            }
        }
    }
}

fn print_outcome(host: &HostInfo, result: &anyhow::Result<Outcome>) -> anyhow::Result<()> {
    let addr = format!("{}:{}", host.host, host.port);

    match result {
        Ok(Outcome::RunCommandOutcome(command_outcome)) => {
            print_command_outcome(&addr, command_outcome)?;
        }
        Ok(Outcome::SendFileOutcome) => {
            println!("{}", Green.paint(format!("[{addr} OK]")));
        }
        Err(err) => {
            println!("{}", Red.paint(format!("[{addr} ERROR: {err:#}]")));
        }
    }

    Ok(())
}

fn print_command_outcome(addr: &str, outcomd: &RunCommandOutcome) -> anyhow::Result<()> {
    if outcomd.exit_status == 0 {
        println!("{}", Green.paint(format!("[{addr} OK]")));
    } else {
        println!("{}", Red.paint(format!("[{addr} ERROR: exit with {}]", outcomd.exit_status)));
    }

    std::io::stdout().write_all(&outcomd.out)?;
    std::io::stdout().write_all(&outcomd.err)?;

    Ok(())
}
