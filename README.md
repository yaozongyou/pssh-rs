pssh-rs is a parallel ssh tool written in rust.

# Example 

run `date` command on 192.168.56.101 and 192.168.56.102:
```bash
./pssh-rs -h "192.168.56.101;192.168.56.102" -uroot -pmypassword -c 'date'
```
