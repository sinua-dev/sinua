const path = require('path');
const { getDefaultConfig, mergeConfig } = require('@react-native/metro-config');

/**
 * Metro configuration
 * https://reactnative.dev/docs/metro
 *
 * @sinua/react-native lives outside this app's root (a sibling package,
 * symlinked into node_modules) -- Metro doesn't watch or resolve outside
 * its project root by default, so the real package directory needs to be
 * added to watchFolders explicitly.
 *
 * @type {import('@react-native/metro-config').MetroConfig}
 */
const sinuaReactNative = path.resolve(__dirname, '..');

// One copy of react / react-native: the sibling package has its own dev
// install (for tsc), and a second react-native in the bundle breaks Fabric's
// view registry. Resolve those two from this app only.
const config = {
  watchFolders: [sinuaReactNative],
  resolver: {
    blockList: [new RegExp(`${path.join(sinuaReactNative, 'node_modules').replace(/[/\\]/g, '[/\\\\]')}[/\\\\](react|react-native)[/\\\\].*`)],
    extraNodeModules: {
      react: path.resolve(__dirname, 'node_modules/react'),
      'react-native': path.resolve(__dirname, 'node_modules/react-native'),
    },
  },
};

module.exports = mergeConfig(getDefaultConfig(__dirname), config);
