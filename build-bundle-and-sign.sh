#!/bin/zsh

set -euo pipefail

cargo bundle --format osx --release

codesign \
    --strict=all \
    --timestamp \
    --strip-disallowed-xattrs \
    --validate-constraint \
    --options kill,hard,library,runtime \
    --force \
    --sign "Developer ID Application: MARCEL BRAUN" \
    --entitlements ./entitlements.plist \
    ./target/release/bundle/osx/Lyrane.app

if codesign -dv --verbose=4 ./target/release/bundle/osx/Lyrane.app 2>&1 | grep "Authority=Developer ID Application" > /dev/null
    then (echo && echo Resulting bundle has been signed.) \
    else (echo && echo Failed bundle signature check.) \
fi