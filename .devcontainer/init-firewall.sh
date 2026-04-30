#!/usr/bin/env bash
# Restrict outbound network to an allowlist. Resolves DNS at script time,
# adds resolved IPs to an ipset, then drops any outbound traffic that's
# not in the set (and not part of an established connection).
#
# Runs as the `node` user; uses sudo for each iptables/ipset call. The
# sudoers NOPASSWD rule (configured in the Dockerfile) covers exactly
# those binaries — no password prompt expected.
#
# Failure modes:
#   - iptables/ipset unavailable (rare in node:20-bookworm, but theoretically
#     possible if the host kernel lacks netfilter modules) → exit 0, container
#     keeps full network access. The postStartCommand wrapper logs a warning.
#   - DNS resolution fails for some hosts → those hosts will be unreachable;
#     the rest of the allowlist still works.

set -euo pipefail

ALLOWLIST=(
  # Anthropic API (for Claude Code)
  api.anthropic.com
  # npm
  registry.npmjs.org
  registry.yarnpkg.com
  # Rust crates
  crates.io
  static.crates.io
  index.crates.io
  # GitHub (git clone, release downloads, raw files)
  github.com
  api.github.com
  codeload.github.com
  raw.githubusercontent.com
  objects.githubusercontent.com
)

if ! command -v iptables >/dev/null 2>&1; then
  echo "iptables not available; skipping firewall setup" >&2
  exit 0
fi

# Try to create or flush the ipset. If the kernel doesn't support it
# (some Docker Desktop configurations) bail out non-fatally.
if sudo ipset list -n 2>/dev/null | grep -qx allowed-domains; then
  sudo ipset flush allowed-domains
else
  if ! sudo ipset create allowed-domains hash:ip family inet hashsize 4096 maxelem 100000 2>/dev/null; then
    echo "ipset create failed (kernel likely lacks ip_set module); skipping firewall" >&2
    exit 0
  fi
fi

# Resolve every allowlisted host and add A records to the set.
for host in "${ALLOWLIST[@]}"; do
  while read -r ip; do
    [ -n "$ip" ] && sudo ipset add allowed-domains "$ip" -exist
  done < <(getent ahosts "$host" 2>/dev/null | awk '/STREAM/ {print $1}' | sort -u)
done

# Reset and apply OUTPUT rules. Default policy DROP, then explicit allows.
sudo iptables -F OUTPUT
sudo iptables -P OUTPUT DROP
sudo iptables -A OUTPUT -o lo -j ACCEPT
sudo iptables -A OUTPUT -d 127.0.0.0/8 -j ACCEPT
sudo iptables -A OUTPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
# DNS (so future lookups inside the container still work)
sudo iptables -A OUTPUT -p udp --dport 53 -j ACCEPT
sudo iptables -A OUTPUT -p tcp --dport 53 -j ACCEPT
# The allowlist
sudo iptables -A OUTPUT -m set --match-set allowed-domains dst -j ACCEPT

echo "firewall active; allowlist: ${ALLOWLIST[*]}"
