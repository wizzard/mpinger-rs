# MPinger

MPinger is a versatile multi-host roundtrip time (ping) measure tool that employs a variety of methods to efficiently ping destination hosts

Supported ping methods:

* ICMP ping
* TCP connection
* HTTP request

## Console client

`mpinger-cli` is a console client:

```bash
Usage: mpinger-cli [OPTIONS]
Options:
  -d, --debug
  -c, --count <COUNT>        Number of pings to send [default: 5]
  -i, --interval <INTERVAL>  Interval between pings in ms [default: 1000]
      --icmp <ICMP>          List of comma separated addresses to perform ICMP pings
      --connect <CONNECT>    List of comma separated addresses to perform TCP connect pings (default port 80)
      --http <HTTP>          List of comma separated addresses to perform HTTP keepalive pings (default port 80)
  -h, --help                 Print help
  ```

Example:

```bash
sudo ./target/debug/mpinger-cli -c 5 --icmp 1.1.1.1,www.google.com,1.2.3.4 --connect 1.1.1.1,www.google.com --http 1.1.1.1
```

![mpinger-cli output](images/mpinger-cli.png)

## TUI client
