# Healoo R8 rules. Most libraries ship their own rules (kotlinx.serialization, Retrofit, OkHttp,
# Auth0, Firebase, Coil); these cover what they can't know about.

# Keep line numbers in crash reports; the mapping file (build/outputs/mapping) turns them back.
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile

# Retrofit reads generic signatures and annotations of the API interface at runtime.
-keepattributes Signature, InnerClasses, EnclosingMethod, RuntimeVisibleAnnotations, RuntimeVisibleParameterAnnotations, AnnotationDefault
-keep,allowobfuscation,allowshrinking interface com.healoo.app.data.** { @retrofit2.http.* <methods>; }
