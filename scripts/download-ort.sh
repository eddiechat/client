#!/usr/bin/env bash
set -euo pipefail

# ORT version must match the version used by ort-sys in Cargo.lock.
# Check with: cargo tree -p ort-sys | head -1
# Then look at the download URLs in ort-sys/build/download/dist.txt
ORT_VERSION="1.24.2"

LIBS_DIR="$(cd "$(dirname "$0")/.." && pwd)/ort-libs"
mkdir -p "$LIBS_DIR"

case "${1:-}" in

  ios)
    URL="https://onnxruntimepackages.z14.web.core.windows.net/pod-archive-onnxruntime-c-${ORT_VERSION}.zip"
    echo "Downloading ORT iOS XCFramework (v${ORT_VERSION})..."
    curl -L "$URL" -o /tmp/ort-ios.zip
    unzip -q -o /tmp/ort-ios.zip -d /tmp/ort-ios
    mkdir -p "$LIBS_DIR/ios"
    cp -r /tmp/ort-ios/onnxruntime.xcframework "$LIBS_DIR/ios/"
    rm -rf /tmp/ort-ios /tmp/ort-ios.zip
    echo "iOS XCFramework -> $LIBS_DIR/ios/onnxruntime.xcframework"
    ;;

  android)
    URL="https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/${ORT_VERSION}/onnxruntime-android-${ORT_VERSION}.aar"
    echo "Downloading ORT Android AAR (v${ORT_VERSION})..."
    curl -L "$URL" -o /tmp/ort-android.aar
    unzip -q -o /tmp/ort-android.aar -d /tmp/ort-android
    mkdir -p "$LIBS_DIR/android"
    cp -r /tmp/ort-android/jni/* "$LIBS_DIR/android/"
    cp -r /tmp/ort-android/headers "$LIBS_DIR/android/headers"
    rm -rf /tmp/ort-android /tmp/ort-android.aar
    echo "Android .so files -> $LIBS_DIR/android"
    ;;

  *)
    echo "Usage: $0 [ios|android]"
    echo ""
    echo "Downloads prebuilt ONNX Runtime binaries for mobile platforms."
    echo "Desktop builds use ort's built-in download strategy and don't need this."
    exit 1
    ;;
esac
