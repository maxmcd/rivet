FROM maxmcd/gstreamer:1.14-buster

RUN apt-get install -y curl
RUN wget -O rustup.sh https://sh.rustup.rs && sh ./rustup.sh -y
ENV PATH=$PATH:/root/.cargo/bin/

WORKDIR /opt/
RUN mkdir -p /opt/src && echo 'fn main() {println!("");}' > /opt/src/main.rs
COPY Cargo.* /opt/
RUN cargo install
RUN cargo build
COPY . /opt/

ENV GST_DEBUG_DUMP_DOT_DIR=/opt/debug/
ENV RUST_LOG=rust_test=info

CMD cargo run

