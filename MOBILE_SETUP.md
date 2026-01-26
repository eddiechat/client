# Mobile Development Setup for eddie.chat

This guide explains how to build eddie.chat for iOS and Android using Tauri 2.

## Prerequisites

### For Both Platforms
- Node.js 18+ and Bun installed
- Rust toolchain (rustup)
- Tauri CLI (`npm install -g @tauri-apps/cli`)

### For iOS Development
- macOS with Xcode 14+ installed
- Xcode Command Line Tools: `xcode-select --install`
- iOS Simulator or physical device
- Apple Developer account (for device deployment)

### For Android Development
- Android Studio with:
  - Android SDK (API 24+)
  - Android NDK
  - Android SDK Build-Tools
- Java Development Kit (JDK) 17
- Set environment variables:
  ```bash
  export ANDROID_HOME=$HOME/Android/Sdk
  export NDK_HOME=$ANDROID_HOME/ndk/<version>
  export PATH=$PATH:$ANDROID_HOME/platform-tools
  ```

## Initial Setup

### 1. Install Rust Mobile Targets

```bash
# For iOS
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim

# For Android
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

### 2. Initialize Mobile Platforms

```bash
# Initialize iOS project (macOS only)
bun run tauri:ios init

# Initialize Android project
bun run tauri:android init
```

This will create:
- `src-tauri/gen/apple/` - iOS Xcode project
- `src-tauri/gen/android/` - Android Gradle project

## Building for Mobile

### iOS Development

```bash
# Run in iOS Simulator
bun run tauri:ios:dev

# Build for release
bun run tauri:ios:build
```

### Android Development

```bash
# Run in Android Emulator or connected device
bun run tauri:android:dev

# Build APK for release
bun run tauri:android:build
```

## Configuration

### iOS Configuration (`src-tauri/tauri.conf.json`)

The iOS bundle configuration supports:
- `developmentTeam`: Your Apple Developer Team ID (required for device deployment)
- `minimumSystemVersion`: Minimum iOS version (set to "13.0")

### Android Configuration (`src-tauri/tauri.conf.json`)

The Android bundle configuration supports:
- `minSdkVersion`: Minimum Android SDK version (set to 24, Android 7.0)

## App Icons

For mobile, you'll need to provide app icons in various sizes. Place them in:
- `src-tauri/icons/` for desktop
- After running `tauri ios init`, iOS icons go in the generated Xcode asset catalog
- After running `tauri android init`, Android icons go in the generated `res/` directories

Run `tauri icon` to generate icons from a source image:
```bash
bun run tauri icon ./path/to/app-icon.png
```

## Troubleshooting

### iOS Build Issues
- Ensure Xcode is up to date
- Check that you've accepted Xcode license: `sudo xcodebuild -license`
- For signing issues, configure your team ID in `tauri.conf.json`

### Android Build Issues
- Verify ANDROID_HOME and NDK_HOME are set correctly
- Ensure you have the required SDK platforms installed via Android Studio
- For Gradle issues, try: `cd src-tauri/gen/android && ./gradlew clean`

### General Issues
- Clear Rust build cache: `cargo clean` in `src-tauri/`
- Reinstall dependencies: `bun install`
- Update Tauri CLI: `npm update -g @tauri-apps/cli`

## Resources

- [Tauri Mobile Documentation](https://v2.tauri.app/start/prerequisites/)
- [iOS Development Guide](https://v2.tauri.app/distribute/app-store/)
- [Android Development Guide](https://v2.tauri.app/distribute/google-play/)
