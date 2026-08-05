#!/usr/bin/env bash
set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
mobile_dir=${QRBEAM_MOBILE_DIR:-"$repo_root/apps/qrbeam_mobile"}
dist_dir=${QRBEAM_DIST_DIR:-"$repo_root/dist"}
max_app_bytes=$((100 * 1024 * 1024))

count_signing_identities() {
  if [[ -n "${QRBEAM_SIGNING_IDENTITIES:-}" ]]; then
    printf '%s\n' "$QRBEAM_SIGNING_IDENTITIES"
    return
  fi
  security find-identity -v -p codesigning 2>/dev/null \
    | awk '/valid identities found/ { print $1; found=1; exit } END { if (!found) print 0 }'
}

count_provisioning_profiles() {
  if [[ -n "${QRBEAM_PROVISIONING_PROFILES:-}" ]]; then
    printf '%s\n' "$QRBEAM_PROVISIONING_PROFILES"
    return
  fi

  local count=0
  local profile_dir
  for profile_dir in \
    "$HOME/Library/MobileDevice/Provisioning Profiles" \
    "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"; do
    if [[ -d "$profile_dir" ]]; then
      count=$((count + $(find "$profile_dir" -maxdepth 1 -type f -name '*.mobileprovision' | wc -l | tr -d ' ')))
    fi
  done
  printf '%s\n' "$count"
}

find_built_app() {
  local app_path="$mobile_dir/build/ios/iphoneos/Runner.app"
  if [[ -d "$app_path" ]]; then
    printf '%s\n' "$app_path"
    return
  fi

  app_path="$mobile_dir/build/ios/archive/Runner.xcarchive/Products/Applications/Runner.app"
  if [[ -d "$app_path" ]]; then
    printf '%s\n' "$app_path"
    return
  fi

  echo 'iOS build completed without a Runner.app' >&2
  return 1
}

check_no_service_url() {
  local runner_binary=$1
  [[ -f "$runner_binary" ]] || return 0
  if command -v rg >/dev/null 2>&1; then
    if rg -a -n 'https?://[^[:space:]]+' "$runner_binary" >/dev/null; then
      echo "HTTP/HTTPS service address found in $runner_binary" >&2
      return 1
    fi
  elif grep -aE 'https?://[^[:space:]]+' "$runner_binary" >/dev/null; then
    echo "HTTP/HTTPS service address found in $runner_binary" >&2
    return 1
  fi
}

(
  cd "$repo_root"
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
)

(
  cd "$mobile_dir"
  flutter test
  flutter analyze
)

mkdir -p "$dist_dir"
signing_identities=$(count_signing_identities)
provisioning_profiles=$(count_provisioning_profiles)
artifact_path=

if (( signing_identities > 0 && provisioning_profiles > 0 )); then
  signing_status=SIGNED
  (
    cd "$mobile_dir"
    flutter build ipa --release
  )
  built_ipa=$(find "$mobile_dir/build/ios/ipa" -maxdepth 1 -type f -name '*.ipa' -print -quit)
  if [[ -z "$built_ipa" ]]; then
    echo 'signed build completed without an IPA' >&2
    exit 1
  fi
  artifact_path="$dist_dir/QRBeam-Alpha1-signed.ipa"
  cp "$built_ipa" "$artifact_path"
else
  signing_status=UNSIGNED
  (
    cd "$mobile_dir"
    flutter build ios --release --no-codesign
  )
  app_path=$(find_built_app)
  artifact_path="$dist_dir/QRBeam-Alpha1-unsigned.ipa"
  package_dir=$(mktemp -d)
  trap 'rm -rf -- "$package_dir"' EXIT
  mkdir -p "$package_dir/Payload"
  cp -R "$app_path" "$package_dir/Payload/Runner.app"
  rm -f -- "$artifact_path"
  (
    cd "$package_dir"
    zip -qry "$artifact_path" Payload
  )
fi

app_path=$(find_built_app)
app_size_bytes=$(( $(du -sk "$app_path" | awk '{ print $1 }') * 1024 ))
if (( app_size_bytes > max_app_bytes )); then
  echo "Runner.app exceeds 100 MB: $app_size_bytes bytes" >&2
  exit 1
fi
check_no_service_url "$app_path/Runner"

ipa_size_bytes=$(wc -c <"$artifact_path" | tr -d ' ')
ipa_sha256=$(shasum -a 256 "$artifact_path" | awk '{ print $1 }')

printf 'SIGNING_STATUS=%s\n' "$signing_status"
printf 'APP_SIZE_BYTES=%s\n' "$app_size_bytes"
printf 'IPA_SIZE_BYTES=%s\n' "$ipa_size_bytes"
printf 'IPA_SHA256=%s\n' "$ipa_sha256"
printf 'IPA_PATH=%s\n' "$artifact_path"
