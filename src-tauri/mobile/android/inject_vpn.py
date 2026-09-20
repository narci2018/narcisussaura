import os
import shutil

def inject_vpn_components():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(base_dir, "..", "..", ".."))
    android_app_dir = os.path.join(project_root, "src-tauri", "gen", "android", "app")
    
    # 1. Copy NarcissusVpnService.kt
    kt_src = os.path.join(base_dir, "NarcissusVpnService.kt")
    kt_dest_dir = os.path.join(android_app_dir, "src", "main", "java", "com", "narcissus", "aura")
    os.makedirs(kt_dest_dir, exist_ok=True)
    shutil.copy2(kt_src, os.path.join(kt_dest_dir, "NarcissusVpnService.kt"))
    print(f"[Android Inject] Copied NarcissusVpnService.kt to {kt_dest_dir}")

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

        with open(manifest_path, "w", encoding="utf-8") as f:
            f.write(content)
        print(f"[Android Inject] Updated AndroidManifest.xml with VPN service and permissions")

    # 3. Patch build.gradle.kts for release signing
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
