# @sinua/react-native

The sinua views for React Native: the native iOS and Android views in a Fabric
component, so the frame loop, painter, voice and power policy are the native ones.

```bash
npm i @sinua/react-native
cd ios && pod install
```

```tsx
import { SinuaOrb, createVoiceSource } from "@sinua/react-native";

const voice = createVoiceSource({ vendor: "mic" });
<SinuaOrb pattern="speaking" voice={voice} style={{ width: 160, height: 160 }} />;
```

- Typed components per object (`SinuaOrb`, `SinuaRing`, …), generated from the parameter
  catalog, plus the generic `SinuaView`.
- Voice vendors are opt-in per platform: CocoaPods subspecs on iOS, the
  `sinua.voiceVendors` Gradle property on Android.
- iOS 15+, Android API 24+, React Native with the New Architecture. Apache-2.0.
