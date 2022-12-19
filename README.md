pssh-rs is a parallel ssh tool written in rust.

## Example 

1. run `date` command on 192.168.56.101 and 192.168.56.102:
```bash
./pssh-rs -h "192.168.56.101;192.168.56.102" -uroot -pmypassword -c 'date'
```

the hosts can be separated by comma, semicolon or spaces. 

2. using toml file to config hosts, suppose we have the following hosts.toml file:

```bash
$ cat ./hosts.toml 
username = "root"
password = "aaa"
port = 22
hosts = [
    "192.168.56.101",
    "192.168.56.102"
]

[nginx]
username = "ubuntu"
password = "bbb"
port = 22
hosts = [
    "192.168.57.101",
    "192.168.57.102"
]

[[nginx.host]]
host = "192.168.57.103"
password = "ccc"
port = 36000

[[nginx.host]]
host = "192.168.57.104"
username = "root"
password = "ddd"
port = 36000
```

execute `date` on all nginx hosts

```bash
./pssh-rs -t hosts.toml -s nginx -c 'date'
```

note that the username of 192.168.57.103 is ubuntu, which inherited from its parent nginx section.

## Install

just run `cargo install pssh-rs` to install.

## Building

pssh-rs can be built with `cargo build --release`, or using the following
command to build statically:

```bash
sudo apt install musl-tools -y
rustup target add x86_64-unknown-linux-musl
cargo build --target=x86_64-unknown-linux-musl --release
```
