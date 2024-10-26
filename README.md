pssh-rs is a parallel ssh tool written in rust.

## Example 

1. Generate config file template:

```bash
./pssh-rs init
```
after this command, `hosts.toml` will be generated at the current directory,
change file contents and using --config ./hosts.toml argument to use this file.

2. run `date` command on default hosts:

```bash
./pssh-rs --config=./hosts.toml --num_threads=10 run 'date'
```

run `date` on all nginx hosts

```bash
./pssh-rs --config=./hosts.toml --num_threads=10 -s nginx run 'date'
```

3. send file to remote hosts:

```bash
./pssh-rs --config=./hosts.toml send ./hello.txt /tmp/
```

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
