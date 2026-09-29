import * as React from "react";
import { Pressable, StyleSheet, Text, View, useColorScheme, type AccessibilityActionEvent, type StyleProp, type ViewStyle } from "react-native";
import { SinuaView } from "./SinuaView";
import type { VoiceSourceHandle } from "./voice";
import {
  DEFAULT_VOICE_BUTTON_LABELS,
  VoiceButtonController,
  voiceButtonHint,
  type VoiceButtonLabels,
  type VoiceButtonMode,
  type VoiceButtonState,
} from "./voiceButton";

/** Hold this long to end the session (also the ✕ and the "End voice session" action). */
export const VOICE_BUTTON_LONG_PRESS_MS = 600;

export interface SinuaVoiceButtonProps {
  /** A handle from `createVoiceSource`; give your `<SinuaView voice>` the same one. */
  voice: VoiceSourceHandle;
  mode?: VoiceButtonMode;
  labels?: Partial<VoiceButtonLabels>;
  /** Diameter in points (default 56). */
  size?: number;
  style?: StyleProp<ViewStyle>;
  onChange?: (state: VoiceButtonState, reason: string | null) => void;
}

/** The inner ring per state (Web's `voiceButtonRing`): an engine view, or none. */
function ring(state: VoiceButtonState): { pattern: string; state: string; overrides: Record<string, number>; speed: number } | null {
  const closed = { progress: 1, strokeWidth: 0.045 };
  if (state === "connecting") return { pattern: "loading", state: "initializing", overrides: { strokeWidth: 0.045, trackOpacity: 0.15 }, speed: 1 };
  if (state === "listening") return { pattern: "completing", state: "listening", overrides: closed, speed: 1 };
  if (state === "muted") return { pattern: "completing", state: "listening", overrides: closed, speed: 0.15 };
  return null;
}

/**
 * A mic button bound to a native voice source (docs/fx-view.md, *Voice button*): ready,
 * connecting, listening, muted or error. Tap connects, then mutes and unmutes (`toggle`),
 * or hold to talk (`pushToTalk`); a long press or the small ✕ ends the session. The
 * native sources sit behind one fan-out each, so this and your `<SinuaView voice>` both
 * follow the same source, and the view shows the mute.
 *
 * ```tsx
 * const voice = React.useMemo(() => createVoiceSource({ vendor: "gemini", credentialUrl }), []);
 * <SinuaView pattern="glowing" voice={voice} />
 * <SinuaVoiceButton voice={voice} />
 * ```
 */
export function SinuaVoiceButton({ voice, mode = "toggle", labels, size = 56, style, onChange }: SinuaVoiceButtonProps) {
  const controller = React.useMemo(() => new VoiceButtonController(voice, mode), [voice]);
  controller.mode = mode;
  const [model, setModel] = React.useState({ state: controller.state as VoiceButtonState, reason: controller.reason });
  const change = React.useRef(onChange);
  change.current = onChange;
  React.useEffect(() => {
    const off = controller.onChange((m) => {
      setModel({ state: m.state, reason: m.reason });
      change.current?.(m.state, m.reason);
    });
    return () => {
      off();
      controller.destroy();
    };
  }, [controller]);

  const dark = useColorScheme() === "dark";
  const text = { ...DEFAULT_VOICE_BUTTON_LABELS, ...labels };
  const { state, reason } = model;
  const live = state === "connecting" || state === "listening" || state === "muted";
  const colors = {
    bg: dark ? "#26262c" : "#f1f1f4",
    fg: dark ? "#ececf0" : "#1c1c21",
    muted: dark ? "#8d8d96" : "#8a8a93",
    error: dark ? "#ef6a6a" : "#c93b3b",
  };
  const tint = state === "error" ? colors.error : state === "muted" ? colors.muted : colors.fg;
  const label = state === "error" && reason ? `${text.error}: ${reason}` : text[state];
  const r = ring(state);
  const longFired = React.useRef(false);

  const onAction = (e: AccessibilityActionEvent) => {
    if (e.nativeEvent.actionName === "activate") controller.assistiveActivate();
    if (e.nativeEvent.actionName === "endSession") controller.end();
  };

  return (
    <View style={[{ width: size, height: size }, style]}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={label}
        accessibilityHint={voiceButtonHint(state, controller.mode, controller.canMute) || undefined}
        accessibilityState={{ selected: state === "muted", busy: state === "connecting" }}
        accessibilityActions={[{ name: "activate" }, ...(live ? [{ name: "endSession", label: text.end }] : [])]}
        onAccessibilityAction={onAction}
        delayLongPress={VOICE_BUTTON_LONG_PRESS_MS}
        onPressIn={() => {
          longFired.current = false;
          if (controller.mode === "pushToTalk") controller.press();
        }}
        onLongPress={() => {
          if (controller.state === "ready" || controller.state === "error") return;
          longFired.current = true;
          controller.end();
        }}
        onPressOut={() => {
          if (controller.mode === "pushToTalk" && !longFired.current) controller.release();
        }}
        onPress={() => {
          if (controller.mode === "toggle" && !longFired.current) controller.press();
        }}
        style={[styles.button, { width: size, height: size, borderRadius: size / 2, backgroundColor: colors.bg }]}
      >
        {r ? (
          // The ring sits on the button's edge and swells outward with the level.
          <View pointerEvents="none" style={[styles.ring, { width: size * 1.28, height: size * 1.28, left: -size * 0.14, top: -size * 0.14 }]}>
            <SinuaView
              pattern={r.pattern}
              state={r.state}
              overrides={r.overrides}
              speed={r.speed}
              voice={voice}
              audioStrength={0.75}
              style={{ width: "100%", height: "100%" }}
              accessibilityElementsHidden
              importantForAccessibility="no-hide-descendants"
            />
          </View>
        ) : null}
        <Icon kind={state === "error" ? "alert" : state === "muted" ? "micOff" : "mic"} box={size * 0.44} color={tint} />
      </Pressable>
      {live ? (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={text.end}
          onPress={() => controller.end()}
          hitSlop={8}
          style={[styles.end, { backgroundColor: colors.bg, borderColor: dark ? "#ffffff2e" : "#0000002e" }]}
        >
          <Text style={{ color: colors.fg, fontSize: 11, fontWeight: "700" }}>✕</Text>
        </Pressable>
      ) : null}
    </View>
  );
}

/**
 * The web element's icons (voice-button.ts) on its 24-unit grid, built from Views so the
 * package needs no SVG dependency: the mic capsule, its cradle and stem, the slash, the alert.
 */
function Icon({ kind, box, color }: { kind: "mic" | "micOff" | "alert"; box: number; color: string }) {
  const u = box / 24;
  const w = 1.8 * u;
  if (kind === "alert") {
    return (
      <View style={{ width: box, height: box, alignItems: "center", justifyContent: "center" }} accessible={false}>
        <View style={{ width: 17 * u, height: 17 * u, borderRadius: 8.5 * u, borderWidth: w, borderColor: color, alignItems: "center", paddingTop: 3.2 * u }}>
          <View style={{ width: w, height: 5.5 * u, borderRadius: w, backgroundColor: color }} />
          <View style={{ width: w, height: w, borderRadius: w, backgroundColor: color, marginTop: 2.2 * u }} />
        </View>
      </View>
    );
  }
  return (
    <View style={{ width: box, height: box }} accessible={false}>
      {/* capsule: x 9..15, y 3..14 */}
      <View style={{ position: "absolute", left: 9 * u, top: 3 * u, width: 6 * u, height: 11 * u, borderRadius: 3 * u, borderWidth: w, borderColor: color }} />
      {/* cradle: the lower half of a 13-wide circle centred at y 11 */}
      <View
        style={{
          position: "absolute",
          left: 5.5 * u,
          top: 11 * u - w / 2,
          width: 13 * u,
          height: 6.5 * u + w / 2,
          borderBottomLeftRadius: 6.5 * u,
          borderBottomRightRadius: 6.5 * u,
          borderWidth: w,
          borderTopWidth: 0,
          borderColor: color,
        }}
      />
      {/* stem: y 17.5..21 */}
      <View style={{ position: "absolute", left: 12 * u - w / 2, top: 17.5 * u, width: w, height: 3.5 * u, borderRadius: w, backgroundColor: color }} />
      {kind === "micOff" ? (
        <View
          style={{
            position: "absolute",
            left: 12 * u - 11.3 * u,
            top: 12 * u - w / 2,
            width: 22.6 * u,
            height: w,
            borderRadius: w,
            backgroundColor: color,
            transform: [{ rotate: "45deg" }],
          }}
        />
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
  button: { alignItems: "center", justifyContent: "center", overflow: "visible" },
  ring: { position: "absolute" },
  end: {
    position: "absolute",
    top: -6,
    right: -6,
    width: 22,
    height: 22,
    borderRadius: 11,
    borderWidth: 1.5,
    alignItems: "center",
    justifyContent: "center",
  },
});
