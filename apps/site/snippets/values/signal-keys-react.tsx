import { SinuaSignal } from "@sinua/web/components";

// The signal patterns' own keys: each is one number from your app.
export function Keys({ quality, accuracy, blips }: { quality: number; accuracy: number; blips: number }) {
  return (
    <>
      {/* How whole the sphere is, 0 to 1: as it falls, points let go and warm to amber. */}
      <SinuaSignal pattern="cohesion" quality={quality} style={{ width: 96, height: 96 }} />
      {/* How tight the cloud is, 0 to 1: 1 is on the core. */}
      <SinuaSignal pattern="halo" accuracy={accuracy} style={{ width: 96, height: 96 }} />
      {/* Stages (2 to 8) and how far along: the clouds light up in turn. */}
      <SinuaSignal pattern="constellation" steps={4} progress={0.5} style={{ width: 96, height: 96 }} />
      {/* Points found (0 to 12): each flashes as the beam reaches it. */}
      <SinuaSignal pattern="scan" blips={blips} style={{ width: 96, height: 96 }} />
    </>
  );
}
