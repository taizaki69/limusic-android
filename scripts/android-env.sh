#!/usr/bin/env bash
# Source this before building the Android app:
#
#     source scripts/android-env.sh
#     ui/node_modules/.bin/tauri android build --apk --target aarch64   # from the repo root
#
# It finds the SDK and NDK and sets what the Tauri CLI does not: rquickjs (under rustypipe) has no
# pregenerated bindings for Android and runs bindgen at build time, which needs the NDK's sysroot.
# See docs/ANDROID.md.

export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
if [ -z "${NDK_HOME:-}" ]; then
	NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -n 1)"
	export NDK_HOME
fi
if [ ! -d "$NDK_HOME" ]; then
	echo "android-env: no NDK under $ANDROID_HOME/ndk; install one with sdkmanager \"ndk;<version>\"" >&2
	return 1 2>/dev/null || exit 1
fi

host="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
sysroot="$NDK_HOME/toolchains/llvm/prebuilt/$host/sysroot"
for target in aarch64_linux_android armv7_linux_androideabi i686_linux_android x86_64_linux_android; do
	export "BINDGEN_EXTRA_CLANG_ARGS_${target}=--sysroot=$sysroot"
done
echo "android-env: SDK $ANDROID_HOME, NDK $NDK_HOME"
