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
    #[structopt(parse(from_os_str), short, long)]
    toml: Option<PathBuf>,

    /// section in toml file
    #[structopt(short = "s", long)]
    section: Option<String>,

    /// hosts which can be separated by comma, semicolon or spaces
    #[structopt(short = "h", long)]
    hosts: Option<String>,

    /// Port
    #[structopt(short = "P", long)]
    port: Option<u16>,

    /// Username
    #[structopt(short = "u", long)]
    username: Option<String>,

    /// Password
    #[structopt(short = "p", long)]
    password: Option<String>,

    /// The command to run remotely
    #[structopt(short = "c", long)]
    pub command: String,

    /// The number of threads.
    #[structopt(long = "num_threads", default_value = "1")]
    pub num_threads: usize,
}

#[derive(Debug)]
pub struct HostInfo {
    pub host: String,
    pub username: String,
    pub password: String,
    pub port: u16,
    pub timeout_ms: u32,
}

impl CommandLineArgs {
    pub fn get_hosts(&self) -> anyhow::Result<Vec<HostInfo>> {
        let using_args =
            self.hosts.is_some() || self.username.is_some() || self.password.is_some() || self.port.is_some();
        let using_toml = self.toml.is_some() || self.section.is_some();

        if using_args && using_toml {
            return Err(anyhow!("using toml file as config, not also setting hosts, username, password or port args"));
        }

        let mut res = vec![];
        if using_args {
            for host in self.hosts.iter().flat_map(|s| s.split(&[',', ';', ' '])).filter(|s| !s.is_empty()) {
                res.push(HostInfo {
                    host: host.to_string(),
                    username: self.username.clone().unwrap_or_else(|| DEFAULT_SSH_USERNAME.to_string()),
                    password: self.password.clone().unwrap_or_default(),
                    port: self.port.unwrap_or(DEFAULT_SSH_PORT),
                    timeout_ms: DEFAULT_SSH_TIMEOUT_MS,
                });
            }
            return Ok(res);
        }

        if using_toml {
            let str = std::fs::read_to_string(self.toml.as_ref().unwrap())?;
            let value = str.parse::<Value>()?;

            let Value::Table(table) = value else {
                return Err(anyhow!("illegal toml format: content of toml should be a table"));
            };

            let Some(ref section) = self.section else {
                return get_hosts_from_table("", &table);
            };

            if section.is_empty() {
                return get_hosts_from_table("", &table);
            }

            let Some(section_value) = table.get(section) else {
                return Err(anyhow!("no {} section in the toml file", section));
            };

            let Value::Table(section_table) = section_value else {
                return Err(anyhow!("illegal section format: content of section should be a table: {}", section));
            };

            return get_hosts_from_table(section, section_table);
        }

        Err(anyhow!("you should using arguments to specify hosts to operate"))
    }
}

fn get_hosts_from_table(section: &str, table: &toml::value::Table) -> anyhow::Result<Vec<HostInfo>> {
    let mut res = vec![];

    let location = get_location(section, "");
    let username = get_username(table.get("username"), &location)?.unwrap_or_else(|| DEFAULT_SSH_USERNAME.to_string());
    let password = get_password(table.get("password"), &location)?.unwrap_or_default();
    let port = get_port(table.get("port"), &location)?.unwrap_or(DEFAULT_SSH_PORT);
    let timeout_ms = get_timeout_ms(table.get("timeout_ms"), &location)?.unwrap_or(DEFAULT_SSH_TIMEOUT_MS);

    for host in table.get("hosts").iter().flat_map(|a| a.as_array()).flatten().flat_map(|v| v.as_str()) {
        res.push(HostInfo {
            username: username.clone(),
            password: password.clone(),
            port,
            host: host.to_string(),
            timeout_ms,
        })
    }

    for value in table.get("host").iter().flat_map(|a| a.as_array()).flatten() {
        let host = get_host(value.get("host"), &location)?;
        if host.is_empty() {
            continue;
        }

        let location = get_location(section, &host);
        let username = get_username(value.get("username"), &location)?.unwrap_or_else(|| username.clone());
        let password = get_password(value.get("password"), &location)?.unwrap_or_else(|| password.clone());
        let port = get_port(value.get("port"), &location)?.unwrap_or(port);
        let timeout_ms = get_timeout_ms(value.get("timeout_ms"), &location)?.unwrap_or(timeout_ms);

        res.push(HostInfo { username, password, port, host, timeout_ms })
    }

    Ok(res)
}

fn get_location(section: &str, host: &str) -> String {
    let section = if section.is_empty() { "default section" } else { section };

    if host.is_empty() {
        format!("[{section}]")
    } else {
        format!("[{section}] host {host}")
    }
}

fn get_port(value: Option<&Value>, location: &str) -> anyhow::Result<Option<u16>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.as_integer().ok_or_else(|| anyhow!("port of {location} should be an u16"))?;
    Ok(Some(value.try_into().context(format!("port of {location} should be in the range [0, 65535]"))?))
}

fn get_timeout_ms(value: Option<&Value>, location: &str) -> anyhow::Result<Option<u32>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.as_integer().ok_or_else(|| anyhow!("timeout_ms of {location} should be an u32"))?;
    Ok(Some(value.try_into().context(format!("timeout_ms of {location} should be valid u32"))?))
}

fn get_username(value: Option<&Value>, location: &str) -> anyhow::Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.as_str().ok_or_else(|| anyhow!("username of {location} should be a string"))?;
    Ok(Some(value.to_string()))
}

fn get_password(value: Option<&Value>, location: &str) -> anyhow::Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.as_str().ok_or_else(|| anyhow!("password of {location} should be a string"))?;
    Ok(Some(value.to_string()))
}

fn get_host(value: Option<&Value>, location: &str) -> anyhow::Result<String> {
    let Some(value) = value else {
        return Err(anyhow!("host of {location} is missing"));
    };

    let value = value.as_str().ok_or_else(|| anyhow!("host of {location} should be a string"))?;
    Ok(value.to_string())
}
