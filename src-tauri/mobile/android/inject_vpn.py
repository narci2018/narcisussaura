import os
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
        service_entry = """        <service
            android:name=".NarcissusVpnService"
            android:permission="android.permission.BIND_VPN_SERVICE"
            android:exported="false"
            android:foregroundServiceType="specialUse">
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

    # 4. Patch build.gradle.kts for release signing
    gradle_path = os.path.join(android_app_dir, "build.gradle.kts")
    if os.path.exists(gradle_path):
        with open(gradle_path, "r", encoding="utf-8") as f:
            gradle_content = f.read()
        if 'signingConfig = signingConfigs.getByName("debug")' not in gradle_content:
            target_str = 'getByName("release") {'
            if target_str in gradle_content:
                gradle_content = gradle_content.replace(
                    target_str,
                    target_str + '\n            signingConfig = signingConfigs.getByName("debug")'
                )
                with open(gradle_path, "w", encoding="utf-8") as f:
                    f.write(gradle_content)
                print("[Android Inject] Configured release signing in build.gradle.kts")

if __name__ == "__main__":
    inject_vpn_components()
