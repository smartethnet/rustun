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
./server
```

it will listen `0.0.0.0:8080`

client1: 

```shell
./client 127.0.0.1:8080 10.0.0.101
```

it will connect server(127.0.0.1:8080)， and set the private ip to 10.0.0.101

client2:

```shell
./client 127.0.0.1:8080 10.0.0.102
```

it will connect server(127.0.0.1:8080)， and set the private ip to 10.0.0.102

3. test

client1 ping client2

10.0.0.101 ping 10.0.0.102

> **note**
> 
> client1 and client2 DONT run in the same machine