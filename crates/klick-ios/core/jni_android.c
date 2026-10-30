// Помощники JNI для jni_android.go: в преамбуле файла с //export определения функций запрещены.
#include <jni.h>

const char *klick_jni_chars(JNIEnv *env, jstring s) {
    return (*env)->GetStringUTFChars(env, s, NULL);
}

void klick_jni_release(JNIEnv *env, jstring s, const char *chars) {
    (*env)->ReleaseStringUTFChars(env, s, chars);
}

jstring klick_jni_string(JNIEnv *env, const char *chars) {
    return (*env)->NewStringUTF(env, chars);
}
