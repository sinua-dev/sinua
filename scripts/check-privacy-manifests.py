#!/usr/bin/env python3
"""Apple privacy manifests: every published Swift target declares what it uses.

    scripts/check-privacy-manifests.py

App Store Connect rejects an upload (ITMS-91053) when a binary calls one of Apple's
"required reason" APIs and no bundle in the app declares a reason for it; an SDK
must carry its own declaration, the app's manifest doesn't cover the SDK. Checked:

1. Every non-test source target of packages/ios, packages/ios-openai and
   packages/ios-livekit has Sources/<T>/PrivacyInfo.xcprivacy, listed in its
   `resources`, and the plist has the four top-level keys.
2. Each required-reason API named in a target's Swift / ObjC sources has its
   category declared in that target's manifest, with a reason Apple lists for it.
3. The engine: `nm -u` of the xcframework's static libraries. Rust std imports
   stat / fstat / fstatat / lstat (backtrace symbolization reads the binary's own
   metadata), so CoreEngine declares FileTimestamp (C617.1). A new import in
   another category fails here before an App Store upload does.
4. The React Native pod is one module: packages/react-native/ios/PrivacyInfo.xcprivacy
   covers its copied Swift plus the engine, and SinuaCore.podspec bundles it.

A category declared but no longer found is a warning, not a failure (a reason
can cover code this scan can't see). Run after packages/ios/build.sh: step 3
reads the xcframework it builds. The API lists are Apple's "Describing use of
required reason API" (checked 2026-10-07).
"""
import glob
import json
import os
import plistlib
import re
import subprocess
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
PACKAGES = ["packages/ios", "packages/ios-openai", "packages/ios-livekit"]
MANIFEST = "PrivacyInfo.xcprivacy"
XCFRAMEWORK = "packages/ios/core_engineFFI.xcframework"
ENGINE_TARGET = ("packages/ios", "CoreEngine")
RN_MANIFEST = "packages/react-native/ios/" + MANIFEST
RN_SOURCES = "packages/react-native/ios"
RN_PODSPEC = "packages/react-native/SinuaCore.podspec"

CAT = "NSPrivacyAccessedAPICategory"
# Apple's approved reasons per category; a typo'd or invented code fails.
REASONS = {
    "FileTimestamp": {"DDA9.1", "C617.1", "3B52.1", "0A2A.1"},
    "SystemBootTime": {"35F9.1", "8FFB.1", "3D61.1"},
    "DiskSpace": {"85F4.1", "E174.1", "7D9E.1", "B728.1"},
    "ActiveKeyboards": {"3EC4.1", "54BD.1"},
    "UserDefaults": {"CA92.1", "1C8F.1", "C56D.1", "AC6B.1"},
}
# Names as they appear in Swift / ObjC sources.
SOURCE_APIS = {
    "FileTimestamp": [
        r"\bcreationDate\b", r"\bmodificationDate\b", r"\bfileModificationDate\b",
        r"\bcontentModificationDateKey\b", r"\bcreationDateKey\b",
        r"\bNSFileCreationDate\b", r"\bNSFileModificationDate\b",
        r"\bNSURLContentModificationDateKey\b", r"\bNSURLCreationDateKey\b",
        r"\bgetattrlist(bulk|at)?\s*\(", r"\bfgetattrlist\s*\(",
        r"\b(f|l)?stat\s*\(", r"\bfstatat\s*\(",
    ],
    "SystemBootTime": [r"\bsystemUptime\b", r"\bmach_absolute_time\b"],
    "DiskSpace": [
        r"\bvolumeAvailableCapacity\w*Key\b", r"\bvolumeTotalCapacityKey\b",
        r"\bNSURLVolume(Available|Total)Capacity\w*Key\b",
        r"\bsystemFreeSize\b", r"\bsystemSize\b", r"\bNSFileSystem(Free)?Size\b",
        r"\bf?statv?fs\s*\(",
    ],
    "ActiveKeyboards": [r"\bactiveInputModes\b"],
    "UserDefaults": [r"\bUserDefaults\b", r"\bNSUserDefaults\b"],
}
# C symbols in the engine's static libraries -> the categories that can cover them
# (Apple lists the getattrlist family under both file timestamps and disk space).
SYMBOLS = {
    "stat": {"FileTimestamp"}, "fstat": {"FileTimestamp"}, "fstatat": {"FileTimestamp"},
    "lstat": {"FileTimestamp"},
    "getattrlist": {"FileTimestamp", "DiskSpace"}, "getattrlistbulk": {"FileTimestamp", "DiskSpace"},
    "fgetattrlist": {"FileTimestamp", "DiskSpace"}, "getattrlistat": {"FileTimestamp", "DiskSpace"},
    "statfs": {"DiskSpace"}, "statvfs": {"DiskSpace"}, "fstatfs": {"DiskSpace"}, "fstatvfs": {"DiskSpace"},
    "mach_absolute_time": {"SystemBootTime"},
}

errors, warnings = [], []


def rel(path):
    return os.path.relpath(path, ROOT)


def declared(manifest_path):
    """Category -> reasons from a manifest; records errors for a malformed one."""
    try:
        with open(manifest_path, "rb") as f:
            doc = plistlib.load(f)
    except (OSError, plistlib.InvalidFileException, ValueError) as e:
        errors.append(f"{rel(manifest_path)}: not a readable plist ({e})")
        return None
    for key in ("NSPrivacyTracking", "NSPrivacyTrackingDomains", "NSPrivacyCollectedDataTypes",
                "NSPrivacyAccessedAPITypes"):
        if key not in doc:
            errors.append(f"{rel(manifest_path)}: missing top-level key {key}")
    out = {}
    for entry in doc.get("NSPrivacyAccessedAPITypes", []):
        cat = entry.get("NSPrivacyAccessedAPIType", "").removeprefix(CAT)
        reasons = set(entry.get("NSPrivacyAccessedAPITypeReasons", []))
        if cat not in REASONS:
            errors.append(f"{rel(manifest_path)}: unknown category {CAT}{cat}")
        elif not reasons:
            errors.append(f"{rel(manifest_path)}: {CAT}{cat} has no reason")
        elif reasons - REASONS[cat]:
            errors.append(f"{rel(manifest_path)}: {CAT}{cat}: reason(s) {sorted(reasons - REASONS[cat])}"
                          f" are not Apple's ({sorted(REASONS[cat])})")
        out[cat] = reasons
    return out


def scan_sources(directory):
    """Category -> 'file:line' hits in the Swift / ObjC sources under directory."""
    hits = {}
    files = [p for ext in ("swift", "m", "mm", "h")
             for p in glob.glob(os.path.join(directory, "**", "*." + ext), recursive=True)]
    for path in sorted(files):
        with open(path, encoding="utf-8") as f:
            for n, line in enumerate(f, 1):
                for cat, patterns in SOURCE_APIS.items():
                    if any(re.search(p, line) for p in patterns):
                        hits.setdefault(cat, []).append(f"{rel(path)}:{n}")
    return hits


def engine_symbols():
    """Categories -> symbols imported by the xcframework's static libraries."""
    libs = glob.glob(os.path.join(ROOT, XCFRAMEWORK, "*", "*.a"))
    if not libs:
        errors.append(f"{XCFRAMEWORK}: no static library; run packages/ios/build.sh first")
        return []
    found = []
    for lib in sorted(libs):
        out = subprocess.run(["nm", "-u", lib], capture_output=True, text=True, check=True).stdout
        for line in out.splitlines():
            name = line.strip().lstrip("_").split("$", 1)[0]
            if name in SYMBOLS:
                found.append((name, SYMBOLS[name], rel(lib)))
    return found


def check(label, decl, needs):
    """needs: list of (acceptable categories, where). decl: category -> reasons."""
    used = set()
    for cats, where in needs:
        if not cats & decl.keys():
            errors.append(f"{label}: uses {' or '.join(sorted(cats))} ({where}) but its manifest"
                          " doesn't declare it")
        used |= cats
    for cat in sorted(decl.keys() - used):
        warnings.append(f"{label}: declares {cat} but no use was found (stale declaration?)")


engine = engine_symbols()
grouped = {}
for name, cats, lib in engine:
    grouped.setdefault(frozenset(cats), set()).add("_" + name)
engine_needs = [(set(cats), f"the engine's xcframework imports {', '.join(sorted(names))}")
                for cats, names in grouped.items()]

for pkg in PACKAGES:
    dump = json.loads(subprocess.run(["swift", "package", "dump-package"], cwd=os.path.join(ROOT, pkg),
                                     capture_output=True, text=True, check=True).stdout)
    for t in dump["targets"]:
        if t["type"] != "regular":
            continue
        name = t["name"]
        src = os.path.join(ROOT, pkg, t.get("path") or f"Sources/{name}")
        label = f"{pkg} target {name}"
        manifest = os.path.join(src, MANIFEST)
        if not os.path.isfile(manifest):
            errors.append(f"{label}: no {rel(manifest)}")
            continue
        if not any(r.get("path") == MANIFEST for r in t.get("resources", [])):
            errors.append(f'{label}: {MANIFEST} is not in its resources (add .process("{MANIFEST}"))')
        decl = declared(manifest)
        if decl is None:
            continue
        needs = [({cat}, ", ".join(where[:3]) + (" ..." if len(where) > 3 else ""))
                 for cat, where in scan_sources(src).items()]
        if (pkg, name) == ENGINE_TARGET:
            needs += engine_needs
        before = len(errors)
        check(label, decl, needs)
        if len(errors) == before:
            print(f"ok  {label}: {', '.join(f'{c} {sorted(r)}' for c, r in sorted(decl.items())) or 'no required-reason API'}")

rn = os.path.join(ROOT, RN_MANIFEST)
if not os.path.isfile(rn):
    errors.append(f"React Native: no {RN_MANIFEST}")
else:
    decl = declared(rn)
    if decl is not None:
        needs = [({cat}, ", ".join(where[:3]) + (" ..." if len(where) > 3 else ""))
                 for cat, where in scan_sources(os.path.join(ROOT, RN_SOURCES)).items()]
        before = len(errors)
        check("React Native pod", decl, needs + engine_needs)
        if len(errors) == before:
            print(f"ok  React Native pod: {', '.join(f'{c} {sorted(r)}' for c, r in sorted(decl.items()))}")
with open(os.path.join(ROOT, RN_PODSPEC), encoding="utf-8") as f:
    if f'"ios/{MANIFEST}"' not in f.read():
        errors.append(f"{RN_PODSPEC}: resource_bundles doesn't include ios/{MANIFEST}")

for w in warnings:
    print("warning: " + w)
if errors:
    print("\n".join("error: " + e for e in errors), file=sys.stderr)
    sys.exit(1)
print(f"privacy manifests: ok ({len(engine)} engine imports covered)")
