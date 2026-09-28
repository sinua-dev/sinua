# The React Native package is not on npm yet. Build it in a clone of the repository
# (see from-source.sh): its build.sh builds the iOS and Android engines and copies them in.
../sinua/packages/react-native/build.sh
# Then add it by path. Metro needs the package's folder in `watchFolders`.
npm i ../sinua/packages/react-native
cd ios && pod install
