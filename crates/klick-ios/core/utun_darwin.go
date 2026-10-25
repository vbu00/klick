package main

import "golang.org/x/sys/unix"

// tunnelFileDescriptor — дескриптор utun, который система открыла для туннеля этого расширения.
// NEPacketTunnelProvider его не отдаёт: ищем среди открытых дескрипторов сокет управления ядра
// `com.apple.net.utun_control` (так делают wireguard-apple и sing-box). -1 — не нашли.
func tunnelFileDescriptor() int {
	info := &unix.CtlInfo{}
	copy(info.Name[:], "com.apple.net.utun_control")
	for fd := 0; fd < 1024; fd++ {
		addr, err := unix.Getpeername(fd)
		if err != nil {
			continue
		}
		ctl, ok := addr.(*unix.SockaddrCtl)
		if !ok {
			continue
		}
		if info.Id == 0 {
			if err := unix.IoctlCtlInfo(fd, info); err != nil {
				continue
			}
		}
		if ctl.ID == info.Id {
			return fd
		}
	}
	return -1
}
