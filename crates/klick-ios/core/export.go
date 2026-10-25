package main

// Вход для Swift и Rust: функции C. Строка ошибки (или NULL) освобождает вызывающий через klickcore_free.

/*
#include <stdlib.h>
*/
import "C"

import (
	"unsafe"

	"github.com/metacubex/mihomo/component/iface"
	"github.com/metacubex/mihomo/component/resolver"
	mc "github.com/metacubex/mihomo/constant"
)

func main() {}

//export klickcore_start
func klickcore_start(home, cfg *C.char, tunFd C.int) *C.char {
	return cError(start(C.GoString(home), C.GoString(cfg), int(tunFd)))
}

//export klickcore_reload
func klickcore_reload() *C.char {
	mu.Lock()
	defer mu.Unlock()
	return cError(apply(config))
}

//export klickcore_stop
func klickcore_stop() {
	mu.Lock()
	defer mu.Unlock()
	stop()
}

// Сеть сменилась (Wi-Fi ↔ сотовая): соединения через прежний интерфейс мертвы, а QUIC-серверы
// (Hysteria2) сами заметят это только по тайм-ауту.
//
//export klickcore_network_changed
func klickcore_network_changed() {
	iface.FlushCache()
	resolver.ResetConnection()
	closeConnections()
}

//export klickcore_version
func klickcore_version() *C.char {
	return C.CString(mc.Version)
}

//export klickcore_free
func klickcore_free(p *C.char) {
	C.free(unsafe.Pointer(p))
}

func cError(err error) *C.char {
	if err == nil {
		return nil
	}
	return C.CString(err.Error())
}
