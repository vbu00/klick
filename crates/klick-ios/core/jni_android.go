//go:build android

package main

// Вход для службы туннеля на Android (Kotlin, класс app.klick.android.TunnelCore): VpnService в своём
// процессе открывает адаптер и отдаёт ядру его дескриптор. Тот же start, что у iOS, — только дескриптор
// не надо искать. Ошибка — строкой, null — всё хорошо.

/*
#include <jni.h>
#include <stdlib.h>

const char *klick_jni_chars(JNIEnv *env, jstring s);
void klick_jni_release(JNIEnv *env, jstring s, const char *chars);
jstring klick_jni_string(JNIEnv *env, const char *chars);
*/
import "C"

import "unsafe"

func goString(env *C.JNIEnv, s C.jstring) string {
	chars := C.klick_jni_chars(env, s)
	defer C.klick_jni_release(env, s, chars)
	return C.GoString(chars)
}

func javaError(env *C.JNIEnv, err error) C.jstring {
	if err == nil {
		return nil
	}
	text := C.CString(err.Error())
	defer C.free(unsafe.Pointer(text))
	return C.klick_jni_string(env, text)
}

//export Java_app_klick_android_TunnelCore_start
func Java_app_klick_android_TunnelCore_start(env *C.JNIEnv, cls C.jclass, home, cfg C.jstring, fd C.jint) C.jstring {
	return javaError(env, start(goString(env, home), goString(env, cfg), int(fd)))
}

//export Java_app_klick_android_TunnelCore_reload
func Java_app_klick_android_TunnelCore_reload(env *C.JNIEnv, cls C.jclass) C.jstring {
	mu.Lock()
	defer mu.Unlock()
	return javaError(env, apply(config))
}

//export Java_app_klick_android_TunnelCore_stop
func Java_app_klick_android_TunnelCore_stop(env *C.JNIEnv, cls C.jclass) {
	mu.Lock()
	defer mu.Unlock()
	stop()
}

//export Java_app_klick_android_TunnelCore_networkChanged
func Java_app_klick_android_TunnelCore_networkChanged(env *C.JNIEnv, cls C.jclass) {
	klickcore_network_changed()
}
