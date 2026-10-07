# SPDX-License-Identifier: Apache-2.0
# Throwaway OpenSSH server for openrows-net integration tests. Credentials and
# host keys are generated per container at start-up; nothing secret is baked in.
FROM alpine:3.22

RUN apk add --no-cache openssh-server-pam socat \
    && adduser -D -s /bin/sh tester \
    && adduser -D -s /bin/sh mfa \
    && rm -f /etc/ssh/ssh_host_*

COPY sshd_config /etc/ssh/sshd_config
COPY entrypoint.sh /usr/local/bin/entrypoint.sh

EXPOSE 22
ENTRYPOINT ["/bin/sh", "/usr/local/bin/entrypoint.sh"]
