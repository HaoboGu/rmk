FROM rust:latest

ENV SHELL=/bin/bash

RUN apt-get update && apt-get install -y \
    bash-completion \
    libclang-dev \
    clang \
    usbutils \
    git \
    curl

RUN apt-get clean -y && \
  apt-get autoremove --purge -y && \
  rm -rf /var/lib/apt/lists/*

RUN cargo install flip-link cargo-make

RUN curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh

RUN mkdir -p /root/.vscode-server/extensions

# Set working directory
WORKDIR /workspace
VOLUME ["/workspace"]

CMD ["/bin/bash"]
