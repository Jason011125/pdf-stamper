FROM node:20-bookworm

ARG TZ=UTC
ENV TZ="$TZ" \
    DEBIAN_FRONTEND=noninteractive

# System packages:
#   - Standard build toolchain (gcc/make/pkg-config/openssl) and QoL utils.
#   - Network tools (iptables/ipset/dnsutils) for the firewall init script.
#   - Tauri 2 Linux build deps (gtk-3, webkit2gtk-4.1, etc.). These are
#     required at *compile* time even though Ralph never runs `tauri dev`
#     — `cargo test` for src-tauri links against tauri crates which
#     transitively pull in gdk-sys / webkit2gtk-sys / soup3-sys. Without
#     these dev libs `cargo test` fails with "system library X not found".
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        dnsutils \
        file \
        git \
        gnupg2 \
        iproute2 \
        ipset \
        iptables \
        jq \
        less \
        libayatana-appindicator3-dev \
        libgtk-3-dev \
        librsvg2-dev \
        libssl-dev \
        libwebkit2gtk-4.1-dev \
        libxdo-dev \
        pkg-config \
        procps \
        sudo \
        unzip \
        vim \
        wget \
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
