# rust tunnel

another rust version vpn tunnel.

**status: developing**

![](./doc/arch.png)

## Features

- Multi platform(Android, iOS, Windows, Mac, Linux)
- Multi tenant
- P2P is supported
- IPv6 direct connect
- High Performance

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

etc/server.toml: 
```shell
[server_config]
listen_addr = "0.0.0.0:8080"

[crypto_config]
xor="rustun"

[route_config]
routes_file = "./etc/routes.json"
```

etc/routes.json
```shell
[
  {
    "cluster": "beijing",
    "identity": "bj-office-gw",
    "private_ip": "10.0.1.1",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": ["192.168.1.0/24", "192.168.2.0/24"]
  },
  {
    "cluster": "beijing",
    "identity": "bj-dev-server",
    "private_ip": "10.0.1.2",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": []
  },
  {
    "cluster": "shanghai",
    "identity": "sh-office-gw",
    "private_ip": "10.0.2.1",
    "mask": "255.255.255.0",
    "gateway": "10.0.2.254",
    "ciders": ["192.168.10.0/24"]
  },
  {
    "cluster": "shanghai",
    "identity": "sh-db-server",
    "private_ip": "10.0.2.2",
    "mask": "255.255.255.0",
    "gateway": "10.0.2.254",
    "ciders": []
  }
]

```

run server

```shell
./server etc/server.toml
```

it will:

- listen `0.0.0.0:8080`
- use xor crypto method with key `rustun`
- add two tenants
  - beijing cluster
    - client1, bj-office-gw
    - client2, bj-dev-server
  - shanghai cluster
    - client1, sh-office-gw
    - client2, sh-db-server

client1: bj-dev-serve

```shell
./client -s 192.168.1.8:8080 -i bj-office-gw
```

it will:

- connect server(192.168.1.8:8080)
- use xor crypto with key `rustun`

client2: branch_a

```shell
./client -s 192.168.1.8:8080 -i bj-dev-server
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

**case1: bj-office-gw ping bj-dev-server private ip**

10.0.1.1 ping 10.0.1.2

> **note**
> 
> client1 and client2 DONT run in the same machine