FROM ekidd/rust-musl-builder:stable as builder

USER root

RUN cargo new --bin rango-load-tester
WORKDIR ./rango-load-tester
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release
RUN rm src/*.rs

ADD --chown=rust:rust . ./

RUN rm ./target/x86_64-unknown-linux-musl/release/deps/rango_load_tester*
RUN cargo build --release

FROM alpine:latest

ARG APP=/usr/src/app

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN addgroup -S $APP_USER \
    && adduser -S -g $APP_USER $APP_USER

RUN apk update \
    && apk add --no-cache upx \
    && rm -rf /var/cache/apk/*

COPY --from=builder /home/rust/src/rango-load-tester/target/x86_64-unknown-linux-musl/release/rango-load-tester ${APP}/rango-load-tester
RUN upx ${APP}/rango-load-tester

RUN chown -R $APP_USER:$APP_USER ${APP}

ENV WS_ADDR ws://host.docker.internal:8080/
ENV AMQP_ADDR amqp://stress_ws:stress_ws@host.docker.internal:5672/%2f
ENV WS_POOL_SIZE 20000
ENV RUST_LOG info

USER $APP_USER
WORKDIR ${APP}

CMD ["./rango-load-tester"]
