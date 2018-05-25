FROM maxmcd/gstreamer:latest

RUN apk add rust cargo

WORKDIR /opt/
RUN mkdir -p /opt/src && echo 'fn main() {println!("");}' > /opt/src/main.rs
COPY Cargo.* /opt/
RUN cargo install
COPY . /opt/
RUN cargo build

CMD cargo run

