use ansi_term::Colour::{Green, Red};
use rayon::prelude::*;
use ssh2::Session;
use std::io::prelude::*;
use std::io::Write;
use std::net::TcpStream;
use structopt::StructOpt;

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::CommandLineArgs::from_args();
    let hosts = args.get_hosts()?;

    rayon::ThreadPoolBuilder::new().num_threads(args.num_threads).build_global().unwrap();

    hosts.par_iter().for_each(|host| {
        let addr = format!("{}:{}", host.host, host.port);
        if let Err(err) = remote_exec_command(&addr, &host.username, &host.password, &args.command) {
            println!("{}", Red.paint(format!("[{addr} ERROR: {err}]")));
        }
    });

    Ok(())
}

fn remote_exec_command(addr: &str, username: &str, password: &str, command: &str) -> anyhow::Result<()> {
    let tcp = TcpStream::connect(addr)?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    sess.userauth_password(username, password)?;

    let mut channel = sess.channel_session()?;
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
    if exit_status == 0 {
        println!("{}", Green.paint(format!("[{addr} OK]")));
    } else {
        println!("{}", Red.paint(format!("[{addr} ERROR: exit with {exit_status}]")));
    }

    std::io::stdout().write_all(&out)?;
    std::io::stdout().write_all(&err)?;

    Ok(())
}
