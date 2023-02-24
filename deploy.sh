#!/usr/bin/env bash

set -e

ARCH="x86_64-linux"
TARGET="nostodon@50.116.47.185"
PROFILE_PATH="/nix/var/nix/profiles/system"

nix build ".#ops.${ARCH}.core-server.config.system.build.toplevel"

RESULT_PATH="$(readlink -f ./result)"

nix-copy-closure -s --to ${TARGET} ${RESULT_PATH}

ssh "${TARGET}" sudo nix-env --profile "${PROFILE_PATH}" --set "${RESULT_PATH}"
ssh "${TARGET}" sudo "${PROFILE_PATH}/bin/switch-to-configuration" switch
