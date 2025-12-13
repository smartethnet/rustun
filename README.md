# rust tunnel

another rust version vpn tunnel.

**status: developing**

![](arch.png)

## How to

1. compile

```shell
cargo build
```

or cross compile

```shell
cross build --target x86_64-unknown-linux-gnu
```

replace x86_64-unknown-linux-gnu wiht your target arch.

2. run

```shell
./server etc/server.toml
```

it will listen `0.0.0.0:8080`

client1: 

```shell
➜  rustun git:(main) ✗ cat etc/client.toml 
[client_config]
server_addr = "127.0.0.1:8080"
key = "client-key1"

[device_config]
private_ip = "10.0.0.101"
mask = "255.255.255.0"
gateway="10.0.0.1"
routes_to_me=[
    "192.168.1.201/32"
]

[crypto_config]
xor="rustun"% 

./client etc/client.toml
```

it will:

- connect server(127.0.0.1:8080)
- set the virtual private ip to **10.0.0.101**
- announce $routes_to_me ciders to private network, destination ip in $routes_to_me will route to client1


client2:

```shell
➜  rustun git:(main) ✗ cat etc/client.toml
[client_config]
server_addr = "127.0.0.1:8080"
key = "client-key2"

[device_config]
private_ip = "10.0.0.102"
mask = "255.255.255.0"
gateway="10.0.0.1"

[crypto_config]
xor="rustun"%       

./client etc/client.toml
```

it will:
- connect server(127.0.0.1:8080)
- set the private ip to **10.0.0.102**

3. test

client1 ping client2

10.0.0.101 ping 10.0.0.102

> **note**
> 
> client1 and client2 DONT run in the same machine