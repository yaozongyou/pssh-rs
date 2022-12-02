#![feature(once_cell)]

use ansi_term::Colour::{Green, Red};
use rayon::prelude::*;
use ssh2::Session;
use std::io::prelude::*;
use std::io::Write;
use std::net::TcpStream;
use std::sync::LazyLock;
use structopt::StructOpt;

static VERSION_INFO: LazyLock<String> = LazyLock::new(|| {
    format!(
        "version: {} {}@{} last modified at {} build at {}",
        env!("VERGEN_GIT_SEMVER"),
        env!("VERGEN_GIT_SHA_SHORT"),
        env!("VERGEN_GIT_BRANCH"),
        env!("VERGEN_GIT_COMMIT_TIMESTAMP"),
        env!("VERGEN_BUILD_TIMESTAMP"),
    )
});

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    name = "pssh-rs",
    about = "pssh-rs",
    long_version = &**VERSION_INFO,
)]
struct CommandLineArgs {
    /// Password
    #[structopt(short = "h", long)]
    hosts: String,

    /// Port
    #[structopt(short = "P", long, default_value = "22")]
    port: u16,

    /// Username
    #[structopt(short = "u", long)]
    username: String,

    /// Password
    #[structopt(short = "p", long)]
    password: String,

    /// Command
    #[structopt(short = "c", long)]
    command: String,

    /// The number of threads.
    #[structopt(long = "num_threads", default_value = "1")]
    num_threads: usize,
}

fn main() {
    let args = CommandLineArgs::from_args();

    rayon::ThreadPoolBuilder::new().num_threads(args.num_threads).build_global().unwrap();

    let hosts = args.hosts.split(&[',', ';', ' ']).collect::<Vec<_>>();
    hosts.par_iter().for_each(|host| {
        let addr = format!("{}:{}", host, args.port);
        if let Err(err) = remote_exec_command(&addr, &args.username, &args.password, &args.command) {
            println!("{}", Red.paint(format!("[{} ERROR: {}]", addr, err)));
        }
    })
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
                println!("failed to read: {}", err);
            }
        });
        s.spawn(|| {
            if let Err(err) = channel.stderr().take(1024 * 1024).read_to_end(&mut err) {
                println!("failed to read: {}", err);
            }
        });
    });

    channel.wait_close()?;

    let exit_status = channel.exit_status()?;
    if exit_status == 0 {
        println!("{}", Green.paint(format!("[{} OK]", addr)));
    } else {
        println!("{}", Red.paint(format!("[{} ERROR: exit with {}]", addr, exit_status)));
    }

    std::io::stdout().write_all(&out)?;
    std::io::stdout().write_all(&err)?;

    Ok(())
}
