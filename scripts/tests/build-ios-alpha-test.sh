#!/usr/bin/env bash
set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
build_script="$repo_root/scripts/build-ios-alpha.sh"

if [[ ! -x "$build_script" ]]; then
  echo "missing executable build script: $build_script" >&2
  exit 1
fi

test_root=$(mktemp -d)
trap 'rm -rf -- "$test_root"' EXIT
fake_bin="$test_root/bin"
fake_mobile="$test_root/mobile"
mkdir -p "$fake_bin" "$fake_mobile"

fake_tool="$fake_bin/fake-tool"
cat >"$fake_tool" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

tool=$(basename -- "$0")
printf '%s %s\n' "$tool" "$*" >>"$QRBEAM_TEST_CALLS"

case "$tool" in
  cargo)
    exit 0
    ;;
  flutter)
    if [[ "${QRBEAM_TEST_FAIL_FLUTTER:-0}" == "1" ]]; then
      exit 23
    fi
    if [[ "$*" == "build ios --release --no-codesign" ]]; then
      mkdir -p build/ios/iphoneos/Runner.app
      printf 'offline qrbeam binary' >build/ios/iphoneos/Runner.app/Runner
    elif [[ "$*" == "build ipa --release" ]]; then
      mkdir -p build/ios/ipa build/ios/iphoneos/Runner.app
      printf 'offline qrbeam binary' >build/ios/iphoneos/Runner.app/Runner
      printf 'signed ipa' >build/ios/ipa/qrbeam_mobile.ipa
    fi
    exit 0
    ;;
  *)
    echo "unexpected fake tool: $tool" >&2
    exit 64
    ;;
esac
EOF
chmod +x "$fake_tool"
ln -s "$fake_tool" "$fake_bin/cargo"
ln -s "$fake_tool" "$fake_bin/flutter"

run_build() {
  local dist_dir=$1
  shift
  env \
    PATH="$fake_bin:$PATH" \
    QRBEAM_MOBILE_DIR="$fake_mobile" \
    QRBEAM_DIST_DIR="$dist_dir" \
    QRBEAM_TEST_CALLS="$test_root/calls.log" \
    "$@" \
    "$build_script"
}

unsigned_dist="$test_root/unsigned-dist"
unsigned_output=$(run_build "$unsigned_dist" \
  QRBEAM_SIGNING_IDENTITIES=0 \
  QRBEAM_PROVISIONING_PROFILES=0)
grep -q '^SIGNING_STATUS=UNSIGNED$' <<<"$unsigned_output"
test -f "$unsigned_dist/QRBeam-Alpha1-unsigned.ipa"
unzip -tq "$unsigned_dist/QRBeam-Alpha1-unsigned.ipa" >/dev/null
grep -q '^flutter build ios --release --no-codesign$' "$test_root/calls.log"

: >"$test_root/calls.log"
signed_dist="$test_root/signed-dist"
signed_output=$(run_build "$signed_dist" \
  QRBEAM_SIGNING_IDENTITIES=1 \
  QRBEAM_PROVISIONING_PROFILES=1)
grep -q '^SIGNING_STATUS=SIGNED$' <<<"$signed_output"
test -f "$signed_dist/QRBeam-Alpha1-signed.ipa"
grep -q '^flutter build ipa --release$' "$test_root/calls.log"

set +e
run_build "$test_root/failing-dist" \
  QRBEAM_SIGNING_IDENTITIES=0 \
  QRBEAM_PROVISIONING_PROFILES=0 \
  QRBEAM_TEST_FAIL_FLUTTER=1 >/dev/null 2>&1
failure_status=$?
set -e
if [[ "$failure_status" -ne 23 ]]; then
  echo "expected Flutter exit code 23, got $failure_status" >&2
  exit 1
fi

echo "build-ios-alpha tests passed"
