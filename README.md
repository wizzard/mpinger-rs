# MPinger

MPinger is a versatile multi-host roundtrip time (ping) measure tool that employs a variety of methods to efficiently ping destination hosts

Supported ping methods:

* ICMP ping
* TCP connection
* HTTP request

## Console client

```bash
cargo build --release  && sudo ./target/release/mpinger-cli -c 0 --icmp 1.1.1.1,www.google.com
```
