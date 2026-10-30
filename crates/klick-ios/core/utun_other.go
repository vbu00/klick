//go:build !darwin

package main

// Не iOS: дескриптор адаптера ядру отдаёт тот, кто его открыл (на Android — VpnService), искать нечего.
func tunnelFileDescriptor() int {
	return -1
}
