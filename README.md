pssh-rs is a parallel ssh tool written in rust.

## Example 

1. run `date` command on 192.168.56.101 and 192.168.56.102:
```bash
./pssh-rs -h "192.168.56.101;192.168.56.102" -uroot -pmypassword -c 'date'
```

the hosts can be separated by comma, semicolon or spaces. 

## Building

pssh-rs can be built with `cargo build --release`, or using the following
command to build statically:

```bash
sudo apt install musl-tools -y
rustup target add x86_64-unknown-linux-musl
cargo build --target=x86_64-unknown-linux-musl --release
```
