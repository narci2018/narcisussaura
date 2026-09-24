import os
import re
import shutil

def inject_vpn_components():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(base_dir, "..", "..", ".."))
    android_app_dir = os.path.join(project_root, "src-tauri", "gen", "android", "app")

    kt_dest_dir = os.path.join(android_app_dir, "src", "main", "java", "com", "narcissus", "aura")
    os.makedirs(kt_dest_dir, exist_ok=True)

    # 1. Copy Kotlin source files
    for kt_file in ["NarcissusVpnService.kt", "VpnInitProvider.kt"]:
        kt_src = os.path.join(base_dir, kt_file)
        if os.path.exists(kt_src):
            shutil.copy2(kt_src, os.path.join(kt_dest_dir, kt_file))
            print(f"[Android Inject] Copied {kt_file} to {kt_dest_dir}")
        else:
            print(f"[Android Inject] WARNING: {kt_file} not found at {kt_src}")

    # 2. Patch AndroidManifest.xml
    manifest_path = os.path.join(android_app_dir, "src", "main", "AndroidManifest.xml")
    if os.path.exists(manifest_path):
        with open(manifest_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Add permissions if not present
        permissions = [
            '<uses-permission android:name="android.permission.INTERNET" />',
            '<uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />',
            '<uses-permission android:name="android.permission.CHANGE_NETWORK_STATE" />',
            '<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />',
            '<uses-permission android:name="android.permission.FOREGROUND_SERVICE_SPECIAL_USE" />',
            '<uses-permission android:name="android.permission.POST_NOTIFICATIONS" />'
        ]

        needed_perms = [p for p in permissions if p not in content]
        if needed_perms:
            perm_block = "\n    " + "\n    ".join(needed_perms)
            if "<application" in content:
                content = content.replace("<application", f"{perm_block}\n\n    <application", 1)

        # Add VpnService inside <application>
        # SUPPORTS_ALWAYS_ON is NOT optional: MIUI's Settings → VPN app list
        # (and AOSP's always-on picker) only surface VpnService declarations
        # carrying this metadata. Without it our app never appears in the
        # system VPN page (v0.2.88 field report), which is the one consent
        # entry point MIUI cannot swallow — the dialog there is raised BY
        # Settings itself. android:label feeds the consent dialog title.
        service_entry = """        <service
            android:name=".NarcissusVpnService"
            android:permission="android.permission.BIND_VPN_SERVICE"
            android:enabled="true"
            android:exported="false"
            android:label="Narcissus Aura"
            android:foregroundServiceType="specialUse">
            <meta-data
                android:name="android.net.VpnService.SUPPORTS_ALWAYS_ON"
                android:value="true" />
            <property
                android:name="android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE"
                android:value="VPN proxy connection and traffic routing" />
            <intent-filter>
                <action android:name="android.net.VpnService" />
            </intent-filter>
        </service>"""

        if "NarcissusVpnService" not in content:
            if "</application>" in content:
                content = content.replace("</application>", f"{service_entry}\n    </application>", 1)

        # Add VpnInitProvider inside <application>
        provider_entry = """        <provider
            android:name=".VpnInitProvider"
            android:authorities="${applicationId}.vpninit"
            android:exported="false" />"""

        if "VpnInitProvider" not in content:
            if "</application>" in content:
                content = content.replace("</application>", f"{provider_entry}\n    </application>", 1)

        with open(manifest_path, "w", encoding="utf-8") as f:
            f.write(content)
        print(f"[Android Inject] Updated AndroidManifest.xml with VPN service, provider, and permissions")

    # 3. Patch MainActivity.kt to prevent WebView from drawing behind system bars
    main_activity_path = os.path.join(kt_dest_dir, "MainActivity.kt")
    if os.path.exists(main_activity_path):
        with open(main_activity_path, "r", encoding="utf-8") as f:
            activity_content = f.read()

        if "setDecorFitsSystemWindows" not in activity_content:
            inset_line = "        androidx.core.view.WindowCompat.setDecorFitsSystemWindows(window, true)"

            if "fun onCreate" in activity_content:
                activity_content = activity_content.replace(
                    "super.onCreate(savedInstanceState)",
                    "super.onCreate(savedInstanceState)\n" + inset_line
                )
            else:
                onCreate_method = (
                    "\n    override fun onCreate(savedInstanceState: android.os.Bundle?) {\n"
                    "        super.onCreate(savedInstanceState)\n"
                    + inset_line + "\n"
                    "    }\n"
                )
                activity_content = activity_content.replace(
                    ") : TauriActivity() {",
                    ") : TauriActivity() {" + onCreate_method
                )

            with open(main_activity_path, "w", encoding="utf-8") as f:
                f.write(activity_content)
            print("[Android Inject] Patched MainActivity.kt for system bar inset handling")

    # 4. Patch build.gradle.kts for release signing with STABLE keystore
    #    (debug keystore is regenerated per CI runner -> ANDROID_ID changes ->
    #     machine_id changes every build. A fixed keystore keeps both stable.)
    keystore_src = os.path.join(base_dir, "narcissus.keystore")
    keystore_dst = os.path.join(android_app_dir, "narcissus.keystore")
    if os.path.exists(keystore_src):
        shutil.copy2(keystore_src, keystore_dst)
        print(f"[Android Inject] Copied stable keystore to {keystore_dst}")

    gradle_path = os.path.join(android_app_dir, "build.gradle.kts")
    if os.path.exists(gradle_path):
        with open(gradle_path, "r", encoding="utf-8") as f:
            gradle_content = f.read()

        if "narcissus.keystore" not in gradle_content:
            m = re.search(r"^([ \t]*)buildTypes \{", gradle_content, re.MULTILINE)
            if not m:
                raise RuntimeError("buildTypes block not found in build.gradle.kts; cannot inject signing config")
            indent = m.group(1)
            signing_block = (
                f"{indent}signingConfigs {{\n"
                f"{indent}    create(\"release\") {{\n"
                f"{indent}        storeFile = file(\"narcissus.keystore\")\n"
                f"{indent}        storePassword = \"narcissus2026\"\n"
                f"{indent}        keyAlias = \"narcissus\"\n"
                f"{indent}        keyPassword = \"narcissus2026\"\n"
                f"{indent}    }}\n"
                f"{indent}}}\n\n"
                f"{m.group(0)}"
            )
            gradle_content = gradle_content[:m.start()] + signing_block + gradle_content[m.end():]

            # Point release build type at our signing config (replace any debug reference)
            gradle_content = gradle_content.replace(
                'signingConfig = signingConfigs.getByName("debug")',
                'signingConfig = signingConfigs.getByName("release")'
            )
            m2 = re.search(r"getByName\(\"release\"\) \{", gradle_content)
            if not m2:
                raise RuntimeError("release buildType not found in build.gradle.kts")
            gradle_content = gradle_content[:m2.end()] + \
                '\n            signingConfig = signingConfigs.getByName("release")' + \
                gradle_content[m2.end():]

            if "narcissus.keystore" not in gradle_content or 'signingConfigs.getByName("release")' not in gradle_content:
                raise RuntimeError("signing injection verification failed in build.gradle.kts")

            with open(gradle_path, "w", encoding="utf-8") as f:
                f.write(gradle_content)
            print("[Android Inject] Configured stable release signing in build.gradle.kts")

        # Force native libs to be extracted to nativeLibraryDir (executable at
        # runtime). For targetSdk>=29 the app data dir is mounted noexec, so
        # exec'ing cores from there fails with EACCES (os error 13).
        if "useLegacyPackaging" not in gradle_content:
            m3 = re.search(r"^([ \t]*)buildTypes \{", gradle_content, re.MULTILINE)
            if not m3:
                raise RuntimeError("buildTypes block not found in build.gradle.kts; cannot inject jniLibs packaging")
            indent = m3.group(1)
            packaging_block = (
                f"{indent}packaging {{\n"
                f"{indent}    jniLibs {{\n"
                f"{indent}        useLegacyPackaging = true\n"
                f"{indent}    }}\n"
                f"{indent}}}\n\n"
                f"{m3.group(0)}"
            )
            gradle_content = gradle_content[:m3.start()] + packaging_block + gradle_content[m3.end():]
            if "useLegacyPackaging = true" not in gradle_content:
                raise RuntimeError("jniLibs packaging injection verification failed in build.gradle.kts")
            with open(gradle_path, "w", encoding="utf-8") as f:
                f.write(gradle_content)
            print("[Android Inject] Enabled jniLibs useLegacyPackaging in build.gradle.kts")

    # 5. Package core binaries as jniLibs pseudo-.so files so the installer
    #    extracts them into nativeLibraryDir, which IS executable.
    #    Rust locate_binary reads native_lib_dir (written by VpnInitProvider)
    #    and prefers lib<name>.so from there.
    binaries_dir = os.path.join(project_root, "src-tauri", "binaries")
    jni_abi_dir = os.path.join(android_app_dir, "src", "main", "jniLibs", "arm64-v8a")
    os.makedirs(jni_abi_dir, exist_ok=True)
    # mihomo is NOT optional: Residential/VPNGate nodes are OpenVPN and run on
    # the mihomo core, so a missing libmihomo.so ships an APK whose residential
    # connect always fails with "mihomo core binary not found" (v0.2.100).
    for bin_name in ["sing-box", "mihomo"]:
        bin_src = os.path.join(binaries_dir, bin_name)
        if os.path.exists(bin_src):
            dst = os.path.join(jni_abi_dir, f"lib{bin_name}.so")
            shutil.copy2(bin_src, dst)
            print(f"[Android Inject] Packaged {bin_name} as {dst}")
        else:
            raise RuntimeError(f"{bin_name} binary not found at {bin_src}; download it in CI before injection")

    # tunrelay (Go gVisor netstack bridging the VpnService fd to sing-box's
    # SOCKS5 port) is mandatory for Android TUN mode — fail loudly if missing.
    relay_src = os.path.join(binaries_dir, "tunrelay")
    if os.path.exists(relay_src):
        shutil.copy2(relay_src, os.path.join(jni_abi_dir, "libtunrelay.so"))
        print(f"[Android Inject] Packaged tunrelay as {os.path.join(jni_abi_dir, 'libtunrelay.so')}")
    else:
        raise RuntimeError(f"tunrelay binary not found at {relay_src}; build it in CI before injection")

if __name__ == "__main__":
    inject_vpn_components()
