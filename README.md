# rust tunnel

another rust version vpn tunnel.

**status: developing**

![](./doc/arch.png)

## How to

1. compile

```shell
cargo build
```

or cross compile

```shell
cross build --target x86_64-unknown-linux-gnu
```

replace x86_64-unknown-linux-gnu with your target arch.

2. run

server side:

```shell
[server_config]
listen_addr = "0.0.0.0:8080"

[crypto_config]
xor="rustun"

[[client_configs]]
identity = "headquarters"
private_ip = "10.0.0.2"
ciders = ["192.168.1.0/24"]

[[client_configs]]
identity = "branch_a"
private_ip = "10.0.0.3"
ciders = []     

./server etc/server.toml
```

it will:

- listen `0.0.0.0:8080`
- use xor crypto method with key `rustun`
- add two clients configurations

client1: headquarters

```shell
./client -s 192.168.1.8:8080 -i headquarters
```

it will:

- connect server(192.168.1.8:8080)
- use xor crypto with key `rustun`

client2: branch_a

```shell
./client -s 192.168.1.8:8080 -i branch_a
```

it will:
- connect server(192.168.1.8:8080)
- use xor crypto with key `rustun`

client usage:

```shell
./client -h
Rustun VPN Client

Usage: client [OPTIONS] --server <SERVER> --identity <IDENTITY>

Options:
  -s, --server <SERVER>
          Server address (e.g., 127.0.0.1:8080)
  -i, --identity <IDENTITY>
          Client identity/name
  -c, --crypto <CRYPTO>
          Encryption method: plain, aes256:<key>, or xor:<key> [default: xor:rustun]
      --keepalive-interval <KEEPALIVE_INTERVAL>
          Keep-alive interval in seconds [default: 10]
      --keepalive-threshold <KEEPALIVE_THRESHOLD>
          Keep-alive threshold (reconnect after this many failures) [default: 5]
  -h, --help
          Print help
  -V, --version
          Print version
```

most of the time you only need to set:

- -s to set which server address to be connect.
- -i client identity, configured in server configurations file, otherwise will be unauthorized.

3. test

**case1: branch_a ping headquarters private ip**

10.0.0.3 ping 10.0.0.2

**case2: branch_a ping client1's ciders ip**

10.0.0.3 ping 192.168.1.201

> **note**
> 
> client1 and client2 DONT run in the same machine