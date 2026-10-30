// Ядро kl!ck для iOS и Android: mihomo библиотекой. `build.sh` собирает его статической библиотекой
// (`-buildmode=c-archive`) для приложения и для расширения VPN, scripts/android/build-core.sh — общей
// библиотекой Android (`-buildmode=c-shared`, вход для службы туннеля — jni_android.go):
//
//   - в расширении — ядро туннеля: конфиг от приложения и адаптер по дескриптору utun, который
//     отдала система (свой адаптер на iOS создать нельзя);
//   - в приложении — проверочное ядро без адаптера: список серверов, задержка, локальная проверка.
//
// Адаптер держит эта обёртка, а не конфиг (как ClashMetaForAndroid): перечитывание конфига через API
// ядра (`PUT /configs`) его не трогает, и туннель не пересоздаётся.
package main

import (
	"errors"
	"fmt"
	"net/netip"
	"os"
	"path/filepath"
	"runtime"
	"runtime/debug"
	"sync"
	"time"

	mc "github.com/metacubex/mihomo/constant"
	"github.com/metacubex/mihomo/hub"
	"github.com/metacubex/mihomo/hub/executor"
	"github.com/metacubex/mihomo/hub/route"
	LC "github.com/metacubex/mihomo/listener/config"
	"github.com/metacubex/mihomo/listener/sing_tun"
	"github.com/metacubex/mihomo/log"
	"github.com/metacubex/mihomo/tunnel"
	"github.com/metacubex/mihomo/tunnel/statistic"
)

// Предел памяти расширения Packet Tunnel на iOS 15+ — 50 МБ на весь процесс, превысил — система его
// убивает. Куче Go оставляем с запасом: остальное — код, стеки, буферы системы.
const (
	goMemoryLimit = 32 << 20
	// Куча выросла до этого — закрыть соединения (их буферы — основная память), пока система не убила туннель.
	emergencyHeap = 40 << 20
)

// Адреса адаптера — те же, что у mihomo по умолчанию: подменные адреса DNS (fake-ip) идут из 198.18.0.0/16.
var (
	tunInet4 = netip.MustParsePrefix("198.18.0.1/30")
	tunInet6 = netip.MustParsePrefix("fdfe:dcba:9876::1/126")
)

// Конфиг туннеля; у проверочного ядра в приложении свои файлы.
const tunnelConfig = "config.yaml"

var (
	mu       sync.Mutex
	tun      *sing_tun.Listener
	config   string
	watching sync.Once
)

// start запускает ядро с конфигом. tunFd: -1 — без адаптера (проверочное ядро), 0 — найти utun
// туннеля в этом процессе самим, больше 0 — этот дескриптор.
func start(home, cfg string, tunFd int) error {
	mu.Lock()
	defer mu.Unlock()
	mc.SetHomeDir(home)
	stop()
	mc.SetConfig(cfg)
	config = cfg
	// Предел памяти — только у расширения iOS (50 МБ на процесс); у службы Android его нет.
	if tunFd >= 0 && runtime.GOOS == "ios" {
		limitMemory()
	}
	if err := apply(cfg); err != nil {
		return err
	}
	if tunFd < 0 {
		return nil
	}
	if tunFd == 0 {
		tunFd = tunnelFileDescriptor()
		if tunFd < 0 {
			stop()
			return errors.New("не найден дескриптор utun туннеля")
		}
	}
	l, err := sing_tun.New(LC.Tun{
		Enable:         true,
		Stack:          mc.TunGvisor,
		DNSHijack:      []string{"any:53", "tcp://any:53"},
		MTU:            9000,
		Inet4Address:   []netip.Prefix{tunInet4},
		Inet6Address:   []netip.Prefix{tunInet6},
		FileDescriptor: tunFd,
	}, tunnel.Tunnel)
	if err != nil {
		stop()
		return fmt.Errorf("адаптер: %w", err)
	}
	tun = l
	return nil
}

// apply читает конфиг и применяет его целиком, вместе со входом управления. Расширение кладёт рядом
// копию применённого конфига: открывшись снова, приложение по ней узнаёт, что у ядра, и подключается
// к нему без перечитывания.
func apply(cfg string) error {
	buf, err := os.ReadFile(cfg)
	if err != nil {
		return err
	}
	parsed, err := executor.ParseWithBytes(buf)
	if err != nil {
		return fmt.Errorf("конфиг: %w", err)
	}
	hub.ApplyConfig(parsed)
	if filepath.Base(cfg) == tunnelConfig {
		_ = writeAtomic(appliedPath(cfg), buf)
	}
	return nil
}

func stop() {
	if tun != nil {
		_ = tun.Close()
		tun = nil
	}
	closeConnections()
	route.ReCreateServer(&route.Config{})
	executor.Shutdown()
	// Пустой конфиг: серверы, правила и входы прежнего ядра закрываются и не держат память. Без
	// запоминания выбора и подменных адресов: иначе ядро откроет cache.db в папке, общей с расширением, и
	// займёт его до конца процесса.
	if empty, err := executor.ParseWithBytes([]byte("mode: direct\nprofile: {store-selected: false, store-fake-ip: false}\n")); err == nil {
		executor.ApplyConfig(empty, true)
	}
	// Папка ядра общая у приложения и расширения: копию применённого конфига убирает только ядро туннеля.
	if filepath.Base(config) == tunnelConfig {
		_ = os.Remove(appliedPath(config))
	}
	runtime.GC()
}

func appliedPath(cfg string) string {
	return filepath.Join(filepath.Dir(cfg), "config.applied")
}

func closeConnections() {
	statistic.DefaultManager.Range(func(c statistic.Tracker) bool {
		_ = c.Close()
		return true
	})
}

func limitMemory() {
	debug.SetMemoryLimit(goMemoryLimit)
	debug.SetGCPercent(50)
	watching.Do(func() {
		go func() {
			var m runtime.MemStats
			for range time.Tick(5 * time.Second) {
				runtime.ReadMemStats(&m)
				if m.HeapInuse+m.StackInuse > emergencyHeap {
					log.Warnln("[kl!ck] памяти %d МБ — закрываю соединения", (m.HeapInuse+m.StackInuse)>>20)
					closeConnections()
					debug.FreeOSMemory()
				}
			}
		}()
	})
}

func writeAtomic(path string, data []byte) error {
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}
