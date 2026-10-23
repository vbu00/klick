package main

import (
	"fmt"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"testing"
	"time"
)

// Ядро в процессе приложения запускается и останавливается много раз (каждая проверка серверов):
// после остановки вход управления закрыт, копия применённого конфига убрана, а следующий запуск работает.
func TestStartStopAgain(t *testing.T) {
	home := t.TempDir()
	cfg := filepath.Join(home, tunnelConfig)
	for round := 1; round <= 2; round++ {
		port := freePort(t)
		body := fmt.Sprintf("mode: rule\nexternal-controller: 127.0.0.1:%d\nsecret: s%d\nrules: [MATCH,DIRECT]\n", port, round)
		if err := os.WriteFile(cfg, []byte(body), 0o600); err != nil {
			t.Fatal(err)
		}
		if err := start(home, cfg, -1); err != nil {
			t.Fatalf("запуск %d: %v", round, err)
		}
		if !answers(port, fmt.Sprintf("s%d", round)) {
			t.Fatalf("запуск %d: API ядра не отвечает", round)
		}
		if applied, err := os.ReadFile(appliedPath(cfg)); err != nil || string(applied) != body {
			t.Fatalf("запуск %d: нет копии применённого конфига: %v", round, err)
		}
		mu.Lock()
		stop()
		mu.Unlock()
		if !gone(port) {
			t.Fatalf("остановка %d: API ядра всё ещё отвечает", round)
		}
		if _, err := os.Stat(appliedPath(cfg)); !os.IsNotExist(err) {
			t.Fatalf("остановка %d: копия применённого конфига осталась", round)
		}
	}
}

// Проверочное ядро в приложении не трогает копию конфига туннеля: папка ядра у них общая.
func TestTesterKeepsTunnelCopy(t *testing.T) {
	home := t.TempDir()
	tunnelCopy := appliedPath(filepath.Join(home, tunnelConfig))
	if err := os.WriteFile(tunnelCopy, []byte("туннель"), 0o600); err != nil {
		t.Fatal(err)
	}
	cfg := filepath.Join(home, "tester.yaml")
	if err := os.WriteFile(cfg, []byte(fmt.Sprintf("mode: rule\nexternal-controller: 127.0.0.1:%d\nsecret: s\n", freePort(t))), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := start(home, cfg, -1); err != nil {
		t.Fatal(err)
	}
	mu.Lock()
	stop()
	mu.Unlock()
	if text, err := os.ReadFile(tunnelCopy); err != nil || string(text) != "туннель" {
		t.Fatal("проверочное ядро стёрло копию конфига туннеля")
	}
}

// Вне расширения VPN дескриптора utun нет.
func TestNoTunnelDescriptorOutsideExtension(t *testing.T) {
	if fd := tunnelFileDescriptor(); fd != -1 {
		t.Fatalf("нашёлся дескриптор %d", fd)
	}
}

func freePort(t *testing.T) int {
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer l.Close()
	return l.Addr().(*net.TCPAddr).Port
}

// Вход управления закрылся (ядро закрывает его не сразу, а в своей горутине).
func gone(port int) bool {
	for i := 0; i < 30; i++ {
		c, err := net.DialTimeout("tcp", fmt.Sprintf("127.0.0.1:%d", port), 200*time.Millisecond)
		if err != nil {
			return true
		}
		c.Close()
		time.Sleep(100 * time.Millisecond)
	}
	return false
}

func answers(port int, secret string) bool {
	client := http.Client{Timeout: time.Second}
	for i := 0; i < 30; i++ {
		req, _ := http.NewRequest("GET", fmt.Sprintf("http://127.0.0.1:%d/version", port), nil)
		req.Header.Set("Authorization", "Bearer "+secret)
		if res, err := client.Do(req); err == nil {
			res.Body.Close()
			return res.StatusCode == http.StatusOK
		}
		time.Sleep(100 * time.Millisecond)
	}
	return false
}
