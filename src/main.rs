use ansi_term::Colour::{Green, Red};
use args::HostInfo;
use rayon::prelude::*;
use ssh2::Session;
use std::io::prelude::*;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc::sync_channel;
use std::time::Duration;
use structopt::StructOpt;

mod args;

struct CommandOutcome {
    exit_status: i32,
    out: Vec<u8>,
    err: Vec<u8>,
}

fn main() -> anyhow::Result<()> {
    let args = args::CommandLineArgs::from_args();
    let hosts = args.get_hosts()?;

    let (sender, receiver) = sync_channel(hosts.len());
    std::thread::scope(|s| {
        s.spawn({
            let hosts = hosts.clone();
            move || {
                let mut command_results = Vec::with_capacity(hosts.len());
                for _ in 0..hosts.len() {
                    command_results.push(None);
                }

                let mut print_index: usize = 0;

                loop {
                    let Ok((index, host, result)) = receiver.recv() else {
                        break;
                    };

                    if !args.keep_stable {
                        print_command_result(host, &result).unwrap();
                        continue;
                    }

                    command_results[index] = Some((host, result));
                    while print_index < command_results.len() {
                        match command_results[print_index] {
                            Some((host, ref result)) => {
                                print_command_result(host, result).unwrap();
                                print_index += 1;
                            }
                            None => {
                                break;
                            }
                        }
                    }
                }
            }
        });

        rayon::ThreadPoolBuilder::new().num_threads(args.num_threads).build_global().unwrap();
        hosts.par_iter().enumerate().for_each(|(index, host)| {
            let result = remote_exec_command(host, &args.command);
            sender.send((index, host, result)).unwrap();
        });

        drop(sender);
    });

    Ok(())
}

fn remote_exec_command(host: &HostInfo, command: &str) -> anyhow::Result<CommandOutcome> {
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
    Ok(CommandOutcome { exit_status, out, err })
}

fn print_command_result(host: &HostInfo, result: &anyhow::Result<CommandOutcome>) -> anyhow::Result<()> {
    let addr = format!("{}:{}", host.host, host.port);

    match result {
        Ok(command_outcome) => {
            print_command_outcome(&addr, command_outcome)?;
        }
        Err(err) => {
            println!("{}", Red.paint(format!("[{addr} ERROR: {err}]")));
        }
    }

    Ok(())
}

fn print_command_outcome(addr: &str, outcomd: &CommandOutcome) -> anyhow::Result<()> {
    if outcomd.exit_status == 0 {
        println!("{}", Green.paint(format!("[{addr} OK]")));
    } else {
        println!("{}", Red.paint(format!("[{addr} ERROR: exit with {}]", outcomd.exit_status)));
    }

    std::io::stdout().write_all(&outcomd.out)?;
    std::io::stdout().write_all(&outcomd.err)?;

    Ok(())
}
