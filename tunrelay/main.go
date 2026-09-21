// tunrelay: userspace TCP/IP relay for Android VpnService TUN fds.
//
// The Kotlin VpnService owns the tunnel interface and hands its fd (same
// process, shared fd table) to this child process. gVisor's netstack
// terminates TCP/UDP on top of that fd and forwards every flow to sing-box's
// SOCKS5 mixed port. sing-box runs as a standalone binary and therefore
// cannot open /dev/net/tun itself (SELinux) — this relay is the bridge.
package main

import (
	"encoding/binary"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"os"
	"os/signal"
	"sync"
	"sync/atomic"
	"syscall"
	"time"

	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/adapters/gonet"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"gvisor.dev/gvisor/pkg/tcpip/link/fdbased"
	"gvisor.dev/gvisor/pkg/tcpip/network/ipv4"
	"gvisor.dev/gvisor/pkg/tcpip/network/ipv6"
	"gvisor.dev/gvisor/pkg/tcpip/stack"
	"gvisor.dev/gvisor/pkg/tcpip/transport/tcp"
	"gvisor.dev/gvisor/pkg/tcpip/transport/udp"
	"gvisor.dev/gvisor/pkg/waiter"
)

var (
	socksServer   string
	tcpForwarder  *tcp.Forwarder
	activeTCP     atomic.Int64
	activeUDP     atomic.Int64
)

type udpSession struct {
	tun  *gonet.UDPConn
	scx  net.Conn
	last atomic.Int64 // unix nano of last activity
}

func (s *udpSession) touch() { s.last.Store(time.Now().UnixNano()) }

var (
	udpMu       sync.Mutex
	udpSessions = map[string]*udpSession{}
)

func main() {
	fd := flag.Int("fd", -1, "already-open TUN file descriptor to inherit")
	mtu := flag.Uint("mtu", 9000, "interface MTU")
	address := flag.String("address", "172.19.0.1", "interface address")
	flag.StringVar(&socksServer, "socks", "127.0.0.1:1080", "sing-box SOCKS5 mixed port")
	flag.Parse()

	log.SetFlags(log.Ltime | log.Lmicroseconds)
	if *fd < 0 {
		log.Fatalf("tunrelay: --fd is required")
	}

	s := stack.New(stack.Options{
		NetworkProtocols:   []stack.NetworkProtocolFactory{ipv4.NewProtocol, ipv6.NewProtocol},
		TransportProtocols: []stack.TransportProtocolFactory{tcp.NewProtocol, udp.NewProtocol},
	})

	linkEP, err := fdbased.New(&fdbased.Options{
		FDs:            []int{*fd},
		MTU:            uint32(*mtu),
		EthernetHeader: false, // Android VpnService delivers raw IP packets
	})
	if err != nil {
		log.Fatalf("tunrelay: fdbased: %v", err)
	}
	if err := s.CreateNIC(1, linkEP); err != nil {
		log.Fatalf("tunrelay: CreateNIC: %v", err)
	}
	// Accept every destination: the tunnel carries traffic for arbitrary
	// remote IPs, all of which are relayed.
	if err := s.SetPromiscuousMode(1, true); err != nil {
		log.Fatalf("tunrelay: promiscuous: %v", err)
	}

	ip := net.ParseIP(*address)
	if ip == nil {
		log.Fatalf("tunrelay: bad --address %q", *address)
	}
	if v4 := ip.To4(); v4 != nil {
		proto := tcpip.ProtocolAddress{
			Protocol:          ipv4.ProtocolNumber,
			AddressWithPrefix: tcpip.AddrFrom4([4]byte(v4)).WithPrefix(),
		}
		if err := s.AddProtocolAddress(1, proto, stack.AddressProperties{}); err != nil {
			log.Fatalf("tunrelay: AddProtocolAddress(v4): %v", err)
		}
	}
	if v6 := ip.To16(); v6 != nil && ip.To4() == nil {
		proto := tcpip.ProtocolAddress{
			Protocol:          ipv6.ProtocolNumber,
			AddressWithPrefix: tcpip.AddrFrom16([16]byte(v6)).WithPrefix(),
		}
		if err := s.AddProtocolAddress(1, proto, stack.AddressProperties{}); err != nil {
			log.Fatalf("tunrelay: AddProtocolAddress(v6): %v", err)
		}
	}

	// Default route: everything goes back out the tunnel NIC.
	var routes []tcpip.Route
	if v4 := ip.To4(); v4 != nil {
		sub, err := tcpip.NewSubnet(tcpip.AddrFrom4([4]byte{0, 0, 0, 0}), tcpip.MaskFromBytes([]byte{0, 0, 0, 0}))
		if err != nil {
			log.Fatal(err)
		}
		routes = append(routes, tcpip.Route{Destination: sub, NIC: 1})
	}
	{
		sub, err := tcpip.NewSubnet(tcpip.AddrFrom16([16]byte{}), tcpip.MaskFromBytes(make([]byte, 16)))
		if err != nil {
			log.Fatal(err)
		}
		routes = append(routes, tcpip.Route{Destination: sub, NIC: 1})
	}
	s.SetRouteTable(routes)

	tcpForwarder = tcp.NewForwarder(s, 0, 4096, handleTCP)
	s.SetTransportProtocolHandler(tcp.ProtocolNumber, tcpForwarder.HandlePacket)
	udpForwarder := udp.NewForwarder(s, handleUDP)
	s.SetTransportProtocolHandler(udp.ProtocolNumber, udpForwarder.HandlePacket)

	go reapUDPSessions()

	sig := make(chan os.Signal, 1)
	signal.Notify(sig, syscall.SIGTERM, syscall.SIGINT)
	log.Printf("tunrelay ready: fd=%d mtu=%d addr=%s socks=%s", *fd, *mtu, *address, socksServer)
	<-sig
	log.Printf("tunrelay: terminating")
}

func endpointAddr(addr tcpip.Address, port uint16) string {
	ip := net.IP(addr.AsSlice())
	if ip.To4() != nil {
		return fmt.Sprintf("%s:%d", ip.String(), port)
	}
	return fmt.Sprintf("[%s]:%d", ip.String(), port)
}

func handleTCP(r *tcp.ForwarderRequest) {
	id := r.ID()
	target := endpointAddr(id.LocalAddress, id.LocalPort)

	var wq waiter.Queue
	ep, err := r.CreateEndpoint(&wq)
	r.Complete(false)
	if err != nil {
		log.Printf("tcp %s: CreateEndpoint: %v", target, err)
		return
	}
	conn := gonet.NewTCPConn(&wq, ep)

	go func() {
		defer conn.Close()
		remote, err := socksConnect(target)
		if err != nil {
			log.Printf("tcp %s: socks: %v", target, err)
			return
		}
		defer remote.Close()
		activeTCP.Add(1)
		defer activeTCP.Add(-1)

		var wg sync.WaitGroup
		wg.Add(1)
		go func() {
			defer wg.Done()
			io.Copy(remote, conn)
			remote.Close()
		}()
		io.Copy(conn, remote)
		conn.CloseRead()
		wg.Wait()
	}()
}

func handleUDP(r *udp.ForwarderRequest) {
	id := r.ID()
	dst := endpointAddr(id.LocalAddress, id.LocalPort)
	src := endpointAddr(id.RemoteAddress, id.RemotePort)
	key := src + "->" + dst

	udpMu.Lock()
	if _, exists := udpSessions[key]; exists {
		udpMu.Unlock()
		return
	}
	udpMu.Unlock()

	var wq waiter.Queue
	ep, err := r.CreateEndpoint(&wq)
	if err != nil {
		log.Printf("udp %s: CreateEndpoint: %v", dst, err)
		return
	}
	uc := gonet.NewUDPConn(&wq, ep)

	scx, aerr := socksAssociate(dst)
	if aerr != nil {
		log.Printf("udp %s: socks associate: %v", dst, aerr)
		uc.Close()
		return
	}
	sess := &udpSession{tun: uc, scx: scx}
	sess.touch()

	udpMu.Lock()
	udpSessions[key] = sess
	udpMu.Unlock()
	activeUDP.Add(1)
	defer activeUDP.Add(-1)

	udpAddr, rerr := net.ResolveUDPAddr("udp", dst)
	if rerr != nil {
		uc.Close()
		scx.Close()
		udpMu.Lock()
		delete(udpSessions, key)
		udpMu.Unlock()
		return
	}

	go func() {
		defer sess.close()
		b := make([]byte, 65536)
		for {
			n, err := uc.Read(b)
			if err != nil {
				return
			}
			sess.touch()
			if _, werr := scx.Write(socksUDPFrame(udpAddr, b[:n])); werr != nil {
				return
			}
		}
	}()

	go func() {
		defer sess.close()
		b := make([]byte, 65536)
		for {
			n, err := scx.Read(b)
			if err != nil || n < header.UDPMinimumSize {
				return
			}
			payload, parseErr := socksUDPUnframe(b[:n])
			if parseErr != nil {
				continue
			}
			sess.touch()
			if _, werr := uc.Write(payload); werr != nil {
				return
			}
		}
	}()
}

func (s *udpSession) close() {
	s.tun.Close()
	s.scx.Close()
	udpMu.Lock()
	defer udpMu.Unlock()
	for k, v := range udpSessions {
		if v == s {
			delete(udpSessions, k)
			break
		}
	}
}

func reapUDPSessions() {
	for {
		time.Sleep(15 * time.Second)
		cutoff := time.Now().Add(-120 * time.Second).UnixNano()
		udpMu.Lock()
		for k, v := range udpSessions {
			if v.last.Load() < cutoff {
				delete(udpSessions, k)
				go v.close()
			}
		}
		udpMu.Unlock()
	}
}

// ---- SOCKS5 client ----

func socksGreeting(c net.Conn) error {
	if _, err := c.Write([]byte{0x05, 0x01, 0x00}); err != nil {
		return err
	}
	b := make([]byte, 2)
	if _, err := io.ReadFull(c, b); err != nil {
		return err
	}
	if b[0] != 0x05 || b[1] != 0x00 {
		return fmt.Errorf("socks greeting rejected: %x", b)
	}
	return nil
}

func socksRequestAddr(target string) ([]byte, error) {
	host, portStr, err := net.SplitHostPort(target)
	if err != nil {
		return nil, err
	}
	portU, err := net.LookupPort("tcp", portStr)
	if err != nil {
		return nil, err
	}
	var port [2]byte
	binary.BigEndian.PutUint16(port[:], uint16(portU))
	ip := net.ParseIP(host)
	if v4 := ip.To4(); v4 != nil {
		return append(append([]byte{0x05, 0x01, 0x00, 0x01}, v4...), port[:]...), nil
	}
	if v6 := ip.To16(); v6 != nil {
		return append(append([]byte{0x05, 0x01, 0x00, 0x04}, v6...), port[:]...), nil
	}
	// Domain fallback (shouldn't happen for tun traffic, but harmless).
	if len(host) > 255 {
		return nil, fmt.Errorf("host too long")
	}
	req := []byte{0x05, 0x01, 0x00, 0x03, byte(len(host))}
	req = append(req, host...)
	return append(req, port[:]...), nil
}

func socksReadReply(c net.Conn) error {
	b := make([]byte, 4)
	if _, err := io.ReadFull(c, b); err != nil {
		return err
	}
	if b[1] != 0x00 {
		return fmt.Errorf("socks failure reply code 0x%02x", b[1])
	}
	switch b[3] {
	case 0x01:
		_, err := io.ReadFull(c, make([]byte, 4+2))
		return err
	case 0x03:
		l := make([]byte, 1)
		if _, err := io.ReadFull(c, l); err != nil {
			return err
		}
		_, err := io.ReadFull(c, make([]byte, int(l[0])+2))
		return err
	case 0x04:
		_, err := io.ReadFull(c, make([]byte, 16+2))
		return err
	}
	return fmt.Errorf("unknown socks ATYP 0x%02x", b[3])
}

func socksDial(target string, cmd byte) (net.Conn, error) {
	c, err := net.DialTimeout("tcp", socksServer, 10*time.Second)
	if err != nil {
		return nil, err
	}
	if err := socksGreeting(c); err != nil {
		c.Close()
		return nil, err
	}
	req, err := socksRequestAddr(target)
	if err != nil {
		c.Close()
		return nil, err
	}
	req[1] = cmd
	if _, err := c.Write(req); err != nil {
		c.Close()
		return nil, err
	}
	if err := socksReadReply(c); err != nil {
		c.Close()
		return nil, err
	}
	return c, nil
}

func socksConnect(target string) (net.Conn, error) { return socksDial(target, 0x01) }

// socksAssociate sets up a UDP_ASSOCIATE session bound for target (used only
// for the 0.0.0.0 placeholder address in the request).
func socksAssociate(_ string) (net.Conn, error) { return socksDial("0.0.0.0:0", 0x03) }

func socksUDPFrame(dst *net.UDPAddr, payload []byte) []byte {
	var head []byte
	if v4 := dst.IP.To4(); v4 != nil {
		head = []byte{0, 0, 0, 0x01}
		head = append(head, v4...)
	} else {
		head = []byte{0, 0, 0, 0x04}
		head = append(head, dst.IP.To16()...)
	}
	var port [2]byte
	binary.BigEndian.PutUint16(port[:], uint16(dst.Port))
	head = append(head, port[:]...)
	out := make([]byte, 0, len(head)+len(payload))
	out = append(out, head...)
	return append(out, payload...)
}

func socksUDPUnframe(b []byte) ([]byte, error) {
	if len(b) < 4 {
		return nil, fmt.Errorf("short frame")
	}
	switch b[3] {
	case 0x01:
		if len(b) < 4+4+2 {
			return nil, fmt.Errorf("short v4 frame")
		}
		return b[10:], nil
	case 0x03:
		domainLen := int(b[4])
		off := 5 + domainLen + 2
		if len(b) < off {
			return nil, fmt.Errorf("short domain frame")
		}
		return b[off:], nil
	case 0x04:
		if len(b) < 4+16+2 {
			return nil, fmt.Errorf("short v6 frame")
		}
		return b[22:], nil
	}
	return nil, fmt.Errorf("bad ATYP")
}
