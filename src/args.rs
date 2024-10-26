use anyhow::{anyhow, Context};
use std::path::PathBuf;
use structopt::StructOpt;
use toml::Value;

const DEFAULT_SSH_PORT: u16 = 22;
const DEFAULT_SSH_USERNAME: &str = "root";
const DEFAULT_SSH_TIMEOUT_MS: u32 = 3000;

#[derive(Clone, Debug, StructOpt)]
#[structopt(name = "pssh-rs", about = "pssh-rs is a parallel ssh tool written in rust")]
pub struct CommandLineArgs {
    /// toml file for config
    #[structopt(parse(from_os_str), short, long, default_value = "./hosts.toml")]
    config: PathBuf,

    /// section in toml file
    #[structopt(short = "s", long)]
    section: Option<String>,

    #[structopt(subcommand)]
    pub command: Command,

    /// The number of threads.
    #[structopt(short, long = "num_threads", default_value = "1")]
    pub num_threads: usize,

    /// Keep the output stable order with designated hosts.
    #[structopt(short = "k", long = "keep_stable")]
    pub keep_stable: bool,
}

#[derive(Clone, Debug, StructOpt)]
pub enum Command {
    /// Init local hosts.toml config file.
    Init,

    /// Run commands on the remote hosts.
    Run {
        /// The command to run remotely
        command: String,
    },

    /// Send file to the remote hosts.
    Send {
        /// local source file path to send
        #[structopt(parse(from_os_str))]
        source_fpath: PathBuf,

        /// destination file path
        #[structopt(parse(from_os_str))]
        target_fpath: PathBuf,
    },
}

#[derive(Clone, Debug)]
pub struct HostInfo {
    pub host: String,
    pub username: String,
    pub password: String,
    pub port: u16,
    pub timeout_ms: u32,
}

impl CommandLineArgs {
    pub fn get_hosts(&self) -> anyhow::Result<Vec<HostInfo>> {
        let str = std::fs::read_to_string(&self.config)?;
        let value = str.parse::<Value>()?;

        let Value::Table(table) = value else {
            return Err(anyhow!("illegal toml format: content of toml should be a table"));
        };

        let Some(ref section) = self.section else {
            return get_hosts_from_table(&table);
        };

        if section.is_empty() {
            return get_hosts_from_table(&table);
        }

        let mut res = vec![];

        let Some(section_value) = table.get(section) else {
            return Err(anyhow!("no {} section in the toml file", section));
        };

        let Value::Table(section_table) = section_value else {
            return Err(anyhow!("illegal section format: content of section should be a table: {}", section));
        };

        let mut hosts = get_hosts_from_table(section_table)?;
        res.append(&mut hosts);

        Ok(res)
    }
}

fn get_hosts_from_table(table: &toml::value::Table) -> anyhow::Result<Vec<HostInfo>> {
    let mut res = vec![];

    let username = get_username(table.get("username"))?.unwrap_or_else(|| DEFAULT_SSH_USERNAME.to_string());
    let password = get_password(table.get("password"))?.unwrap_or_default();
    let port = get_port(table.get("port"))?.unwrap_or(DEFAULT_SSH_PORT);
    let timeout_ms = get_timeout_ms(table.get("timeout_ms"))?.unwrap_or(DEFAULT_SSH_TIMEOUT_MS);

    for host in table.get("hosts").iter().flat_map(|a| a.as_array()).flatten().flat_map(|v| v.as_str()) {
        res.push(HostInfo {
            username: username.clone(),
            password: password.clone(),
            port,
            host: host.to_string(),
            timeout_ms,
        })
    }

    Ok(res)
}

fn get_username(value: Option<&Value>) -> anyhow::Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.as_str().ok_or_else(|| anyhow!("username should be a string"))?;
    Ok(Some(value.to_string()))
}

fn get_password(value: Option<&Value>) -> anyhow::Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.as_str().ok_or_else(|| anyhow!("password should be a string"))?;
    Ok(Some(value.to_string()))
}

fn get_port(value: Option<&Value>) -> anyhow::Result<Option<u16>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.as_integer().ok_or_else(|| anyhow!("port should be an u16"))?;
    Ok(Some(value.try_into().context("port should be in the range [0, 65535]")?))
}

fn get_timeout_ms(value: Option<&Value>) -> anyhow::Result<Option<u32>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.as_integer().ok_or_else(|| anyhow!("timeout_ms should be an u32"))?;
    Ok(Some(value.try_into().context("timeout_ms should be valid u32")?))
}
