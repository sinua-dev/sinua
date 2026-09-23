require "json"

package = JSON.parse(File.read(File.join(__dir__, "package.json")))

Pod::Spec.new do |s|
  s.name         = "SinuaCore"
  s.version      = package["version"]
  s.summary      = package["description"]
  s.license      = { :type => "Apache-2.0", :file => "LICENSE" }
  s.author       = { "Devin" => "" }
  s.homepage     = "https://github.com/devin/sinua"
  s.platform     = :ios, "15.0"
  s.source       = { :git => "https://github.com/devin/sinua.git", :tag => "v#{s.version}" }
  s.swift_version = "5.9"

  # Own bridge files, plus the UniFFI-generated Swift bindings -- copied in
  # by build.sh from packages/ios's build output (no second cross-compile).
  # SinuaView (the SwiftUI SinuaView), SinuaVoice and SinuaVoiceTypes are copied
  # in the same way, with their module imports stripped: the pod is one Swift module.
  s.source_files = "ios/*.{h,m,mm,swift}", "ios/CoreEngine/*.swift", "ios/Sinua/*.swift",
                   "ios/SinuaVoiceTypes/*.swift", "ios/SinuaVoice/*.swift", "ios/Vendors/GeminiLive/*.swift", "ios/Vendors/ElevenLabs/*.swift"
  s.vendored_frameworks = "core_engineFFI.xcframework"
  s.frameworks = "SwiftUI", "AVFoundation", "Accelerate"

  # Voice vendors are opt-in (docs/fx-view.md, *React Native*). Gemini Live and
  # ElevenLabs are Foundation-only and ship above; LiveKit and OpenAI Realtime pull
  # real SDKs, so each is its own subspec. An app adds, in its Podfile:
  #   source "https://github.com/livekit/podspecs.git"   # LiveKit's own spec repo
  #   pod "SinuaCore/LiveKit"   # and/or "SinuaCore/OpenAI"
  # Without the subspec, creating that vendor's source fails with this instruction
  # instead of linking an SDK the app never asked for.
  s.default_subspecs = "Core"

  s.subspec "Core" do |core|
    core.source_files = "ios/Empty.swift"
  end

  s.subspec "LiveKit" do |livekit|
    livekit.source_files = "ios/Vendors/LiveKit/*.swift"
    livekit.dependency "LiveKitClient", "~> 2.17"
    # LiveKit's CocoaPods build includes its nanopb headers with <angled> paths, which
    # only resolve with its public header dir on the search path of whatever builds the
    # module -- under explicit modules, that's this target.
    livekit.pod_target_xcconfig = {
      "SWIFT_ACTIVE_COMPILATION_CONDITIONS" => "$(inherited) SINUA_LIVEKIT",
      "HEADER_SEARCH_PATHS" => "$(inherited) \"${PODS_ROOT}/Headers/Public/LiveKitClient\"",
    }
  end

  s.subspec "OpenAI" do |openai|
    openai.source_files = "ios/Vendors/OpenAI/*.swift"
    openai.dependency "LiveKitWebRTC", "~> 150.7871"
    openai.pod_target_xcconfig = { "SWIFT_ACTIVE_COMPILATION_CONDITIONS" => "$(inherited) SINUA_OPENAI" }
  end

  # The Fabric component (SinuaViewComponentView, Codegen spec
  # src/specs/SinuaViewNativeComponent.ts): React-Core, Codegen, Folly, ...
  install_modules_dependencies(s)
end
