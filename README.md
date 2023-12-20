## Logisim
An Accelerated Circuit Simulator.

## How to Run
There are not currently any pre-built binaries/APKS for the app, so for any platform, it must be built from source.

## Compiling From Source
To compile the code for any platform you will need `cargo` from the [Rust toolchain](https://www.rust-lang.org/).
Below are the platforms that the app is currently implemented(or planned) for.

#### Desktop
To compile and run the app on Windows, Linux, or MacOS:
```sh
git clone "https://github.com/MasonFeurer/Logisim.git"
cd Logisim
cargo r -rp logisim-desktop
```

On Linux, you may have to install a few packages first:
```sh
sudo apt install libglib2.0-dev
sudo apt install libatk1.0-dev
sudo apt install libcairo2-dev
sudo apt install libpango1.0-dev
sudo apt install librust-gdk-dev
```

#### Web
There are plans to integrate the app for the web, but there has currently been no progress towards this.

#### Android
- Take a look at `android/setup-compile.sh` to see what needs to be done before the android integration can be built.

To compile the app into an APK:
```sh
git clone "https://github.com/MasonFeurer/Logisim.git"
cd Logisim/android
cargo ndk -t arm64-v8a -o app/src/main/jniLibs/  build
cargo ndk -t armeabi-v7a -o app/src/main/jniLibs/  build
./gradlew build
```
The resulting apk will be at `android/app/build/outputs/apk/debug/app-debug.apk`.

Then you can install and run it on a connected android device (with USB debugging enabled):
```sh
./gradlew installDebug
adb shell am start -n com.logisim.android/.MainActivity

APK_UID=$(adb shell pm list package -U com.logisim.android)
APK_UID_TRIMMED=${APK_UID#*uid:}
adb logcat -c
adb -d logcat -v color --uid $APK_UID_TRIMMED
```

#### IOS
There are plans to integrate the app for IOS, but there has currently been no progress towards this.

## Creating New Integration
The app is structured in a way such that it can be integerated into any application that can render graphics with `wgpu`.
The UI rendering and circuit simulation is all handled in `logisim-common`, and application lifetime is handled by the integration.
You can look at `logisim-desktop` or `logisim-android` for an example on how to integrate the common library.
