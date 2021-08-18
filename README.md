## Rango load testing utility

Load testing utility for [rango](https://github.com/bitzlato/rango) websocket server.

### Configuration

| VARIABLE | DEFAULT | DESCRIPTION |
|----------|---------|-------------|
| WS_POOL_SIZE | 20000 | Number of websocket clients to create |
| MSG_COUNT | 1000 | Number of messages to send |
| MSG_DELAY | 30 | Delay between sending two messages (ms) |
| WS_ADDR | ws://localhost:8080/ | Address to connect to the websocket server (rango) |
| AMQP_ADDR | amwp://localhost:5672/%2f | Address to connect to the rabbitmq server | 

Notice that `MSG_COUNT` is the number of messages to be sent. As each sent message is broadcasted to each client the total number of _received_ messages will be `MSG_COUNT * WS_POOL_SIZE`. E.g. 20_000_000 when default variable values are used.

### Usage

#### Binary

**precondition**: rabbitmq server and rango should be both up and running before executing the tester. 

``` sh
# build the binary
cargo build --release

# execute the tester
WS_POOL_SIZE=500 MSG_COUNT=1000 MSG_DELAY=0 cargo run --release
```

Keep in mind that total number of clients is limited to the number of available ports OS can provide. Inside a container it's 28232 (32768 - 60999 range).

#### Local containerized version

Builds and runs current directory inside a docker container. RabbitMQ server is provided along inside the docker container.

**precondition**: rango should be running before executing the command. Local version of the rabbitmq server should not be running on ports 5672, 15672. Otherwise the port collision will prevent the containerized rabbitmq server to start.

``` sh
docker-compose -f docker-compose.local.yml up
```

### Remote containerized version

Starts RabbitMQ server and 5 containers with 20000 client connections each (total: 100_000 simultaneous connections).

Has the same preconditions as the local containerized version.

``` sh
docker-compose up
```

## Measurements

#### 100k clients, 1000 messages. Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 32Gb RAM, NVMe SSD
_rango server and clients running on the same machine_

``` text
worker2_1   | INFO rango_load_tester: Received 20000000 messages
worker2_1   | INFO rango_load_tester: Mean delivery time is 163193.80485250262ms
...
worker5_1   | INFO rango_load_tester: Received 20000000 messages
worker5_1   | INFO rango_load_tester: Mean delivery time is 168410.05409856033ms
...
worker4_1   | INFO rango_load_tester: Received 20000000 messages
worker4_1   | INFO rango_load_tester: Mean delivery time is 172133.4831429799ms
...
worker1_1   | INFO rango_load_tester: Received 20000000 messages
worker1_1   | INFO rango_load_tester: Mean delivery time is 179832.35141856552ms
...
worker3_1   | INFO rango_load_tester: Received 20000000 messages
worker3_1   | INFO rango_load_tester: Mean delivery time is 192133.97007096457ms
```

