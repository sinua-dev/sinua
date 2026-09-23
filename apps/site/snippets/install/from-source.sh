# Until the packages are published, build them from the repository.
git clone https://github.com/sinua-dev/sinua.git sinua && cd sinua

# Web: build the engine (Rust + wasm-pack) and the packages, then depend on them by path.
(cd packages/core && npm install && npm run build)
(cd packages/web && npm install && npm run build)
(cd packages/voice && npm install && npm run build)
# in your app:  npm i ../sinua/packages/core ../sinua/packages/web ../sinua/packages/voice

# iOS: build the engine's xcframework, then add packages/ios as a local Swift package
# (Xcode: File > Add Package Dependencies... > Add Local...). Products: SinuaView, SinuaVoice.
packages/ios/build.sh

# Android: build the engine's .so files, then include the Gradle build from your settings.gradle.kts:
#   includeBuild("../sinua/packages/android")
packages/android/build.sh
