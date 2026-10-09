#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Applies per-container synthetic credentials (TEST_PASSWORD, AUTHORIZED_KEYS),
# starts loopback-only echo services on 127.0.0.1:7000 and the Unix socket
# /tmp/openrows-echo.sock, and runs sshd.
set -eu

ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_ed25519_key

for user in tester mfa; do
    printf '%s:%s\n' "$user" "$TEST_PASSWORD" | chpasswd
    mkdir -p "/home/$user/.ssh"
    printf '%s\n' "$AUTHORIZED_KEYS" > "/home/$user/.ssh/authorized_keys"
    chmod 700 "/home/$user/.ssh"
    chmod 600 "/home/$user/.ssh/authorized_keys"
    chown -R "$user:$user" "/home/$user/.ssh"
done

# Each connection first receives the container hostname, then an echo of its input,
# so tests can tell which hop's loopback service they reached.
socat TCP-LISTEN:7000,bind=127.0.0.1,fork,reuseaddr SYSTEM:'hostname; exec cat' &
socat UNIX-LISTEN:/tmp/openrows-echo.sock,fork,mode=666 SYSTEM:'hostname; exec cat' &
exec /usr/sbin/sshd.pam -D -e
