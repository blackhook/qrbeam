#!/usr/bin/env bash
set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
mobile_dir=${QRBEAM_MOBILE_DIR:-"$repo_root/apps/qrbeam_mobile"}
dist_dir=${QRBEAM_DIST_DIR:-"$repo_root/dist"}
max_app_bytes=$((100 * 1024 * 1024))

if ! command -v flutter >/dev/null 2>&1; then
  echo 'Flutter is required to build the Android APK' >&2
  exit 1
fi

if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then
  echo 'ANDROID_HOME or ANDROID_SDK_ROOT must point to an installed Android SDK' >&2
  exit 1
fi

(
  cd "$mobile_dir"
  flutter test
  flutter analyze
  flutter build apk --release
)

built_apk=$(find "$mobile_dir/build/app/outputs/flutter-apk" -maxdepth 1 -type f -name '*release*.apk' -print -quit)
if [[ -z "$built_apk" ]]; then
  echo 'Android release build completed without an APK' >&2
  exit 1
fi

apk_size_bytes=$(wc -c <"$built_apk" | tr -d ' ')
if (( apk_size_bytes > max_app_bytes )); then
  echo "APK exceeds 100 MB: $apk_size_bytes bytes" >&2
  exit 1
fi

mkdir -p "$dist_dir"
artifact_path="$dist_dir/QRBeam-Alpha1-android.apk"
cp "$built_apk" "$artifact_path"
apk_sha256=$(shasum -a 256 "$artifact_path" | awk '{ print $1 }')

printf 'SIGNING_STATUS=DEBUG_SIGNED\n'
printf 'APK_SIZE_BYTES=%s\n' "$apk_size_bytes"
printf 'APK_SHA256=%s\n' "$apk_sha256"
printf 'APK_PATH=%s\n' "$artifact_path"
