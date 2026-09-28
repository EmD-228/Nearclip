# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# Tauri's Android runtime reaches this app's own plugin entirely by reflection:
# the class is instantiated by name from Rust, its @Command methods are invoked
# by name, and its @InvokeArg classes are deserialized field by field. Nothing
# in the app references any of it statically, so R8 would strip it. Same rules
# the bundled plugins ship for themselves.
-keep @app.tauri.annotation.TauriPlugin class com.nearclip.** { *; }
-keep @app.tauri.annotation.InvokeArg class com.nearclip.** { *; }

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile