#![feature(once_cell)]

use ansi_term::Colour;
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
    #[structopt(short = "P", long)]
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
        remote_exec_command(&format!("{}:{}", host, args.port), &args.username, &args.password, &args.command);
    })
}

fn remote_exec_command(addr: &str, username: &str, password: &str, command: &str) {
    let tcp = TcpStream::connect(addr).unwrap();
    let mut sess = Session::new().unwrap();
    sess.set_tcp_stream(tcp);
    sess.handshake().unwrap();
    sess.userauth_password(username, password).unwrap();

    let mut channel = sess.channel_session().unwrap();
    channel.exec(command).unwrap();

    let mut out = vec![];
    let mut err = vec![];

    std::thread::scope(|s| {
        s.spawn(|| {
            channel.stderr().take(1024 * 1024).read_to_end(&mut err).unwrap();
        });
        s.spawn(|| {
            channel.stream(0).take(1024 * 1024).read_to_end(&mut out).unwrap();
        });
    });

    channel.wait_close().unwrap();
    if channel.exit_status().unwrap() == 0 {
        println!("{}", Colour::Green.paint(format!("[{} OK]", addr)));
    } else {
        println!("{}", Colour::Red.paint(format!("[{} ERROR]", addr)));
    }

    std::io::stdout().write_all(&out).unwrap();
    std::io::stdout().write_all(&err).unwrap();
}
