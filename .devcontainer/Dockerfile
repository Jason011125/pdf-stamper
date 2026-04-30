FROM node:20-bookworm

ARG TZ=UTC
ENV TZ="$TZ" \
    DEBIAN_FRONTEND=noninteractive

# System packages: build toolchain, network tools for the firewall script,
# and a few QoL utilities. Tauri GUI deps (webkit2gtk, libgtk-3) are
# intentionally omitted — Ralph only runs cargo test / tsc / vitest in
# this container, never `tauri dev`.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        dnsutils \
        git \
        gnupg2 \
        iproute2 \
        ipset \
        iptables \
        jq \
        less \
        libssl-dev \
        pkg-config \
        procps \
        sudo \
        unzip \
        vim \
    && rm -rf /var/lib/apt/lists/*

# Allow the existing `node` user (UID 1000) to run iptables/ipset without
# password, so the firewall init script can be invoked from postStartCommand.
RUN echo "node ALL=(root) NOPASSWD: /usr/sbin/iptables, /usr/sbin/ipset, /usr/sbin/ip6tables, /usr/sbin/iptables-save, /usr/sbin/iptables-restore, /usr/sbin/ip" \
        > /etc/sudoers.d/node-firewall \
    && chmod 0440 /etc/sudoers.d/node-firewall

# Install Claude Code CLI globally (as root, before dropping to node user)
RUN npm install -g @anthropic-ai/claude-code

USER node
WORKDIR /home/node

# Install Rust toolchain via rustup, scoped to the node user
ENV RUSTUP_HOME=/home/node/.rustup \
    CARGO_HOME=/home/node/.cargo \
    PATH=/home/node/.cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal --component clippy,rustfmt

WORKDIR /workspace
