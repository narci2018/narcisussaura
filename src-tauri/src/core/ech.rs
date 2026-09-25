//! Encrypted ClientHello configs for the mihomo core.
//!
//! Why this exists: every link in this fleet carries `ech=cloudflare-ech.com+
//! <resolver>`, and sing-box is given only the *name* and resolves the HTTPS
//! record itself. mihomo has to be handed the **ECHConfigList bytes**: its own
//! DNS answer never carries the `ech` parameter, and `ech-opts.query-server-name`
//! with the lane's DNS off fails every dial.
//!
//! The bytes must be current ones. Measured 2026-09-26 against the bundled
//! v1.19.30 and the user's own Hong Kong node: a day-old config (Cloudflare
//! rotates them) answered 0/7 dials — every request died at the full 12s timeout,
//! i.e. the relay reads as 中转不可用 — while the freshly resolved one returned
//! 204 in 0.21-0.35s on the same node. So this module looks the config up over
//! UDP on every lane build (cached 10 minutes) and the caller omits `ech-opts`
//! entirely when the lookup fails: no ECH is a slower relay, a stale ECH is a
//! dead one.

use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine;

/// AliDNS and DNSPod answer HTTPS records with the `ech` parameter intact and
/// are reachable from inside China; Google/Cloudflare are kept as a last resort
/// for networks where the Chinese resolvers are the ones filtered.
const RESOLVERS: [&str; 3] = [
    "223.5.5.5:53",
    "119.29.29.29:53",
    "8.8.8.8:53",
];

/// Ten minutes is the compromise: a restart-warm app does not re-query on every
/// lane build, and a rotated key is picked up inside one coffee break.
const CACHE_TTL: Duration = Duration::from_secs(600);
/// The lookup can happen on an async worker (the lane builders are sync), so the
/// whole sweep is kept to a couple of seconds even when every resolver is dead.
const QUERY_TIMEOUT: Duration = Duration::from_millis(800);

/// `cloudflare-ech.com` -> base64 ECHConfigList, with the moment it was fetched.
fn cache() -> &'static Mutex<HashMap<String, (String, Instant)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (String, Instant)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The `ech` link parameter is either a server name to query (this is what the
/// panels emit: `ech=cloudflare-ech.com+https://dns.alidns.com/dns-query`) or the
/// base64 config itself.
pub fn config_for(ech: &str) -> Option<String> {
    let (value, inline) = classify(ech)?;
    // Inline config: long, and not a hostname.
    if inline {
        return Some(value.to_string());
    }

    if let Some((cached, fetched)) = cache().lock().ok()?.get(value) {
        if fetched.elapsed() < CACHE_TTL {
            return Some(cached.clone());
        }
    }
    let fetched = query_over_udp(value)?;
    cache()
        .lock()
        .ok()?
        .insert(value.to_string(), (fetched.clone(), Instant::now()));
    Some(fetched)
}

/// The `ech` link parameter is either a server name to query (what these panels
/// emit: `ech=cloudflare-ech.com+https://dns.alidns.com/dns-query`, the resolver
/// riding after the `+` is sing-box's business) or the base64 config itself.
fn classify(ech: &str) -> Option<(&str, bool)> {
    let value = strip_resolver(ech.trim());
    if value.is_empty() {
        return None;
    }
    Some((value, !looks_like_host(value)))
}

/// Cut the `+<resolver url>` tail off a query name. Only a `+` that starts a URL
/// counts — a bare base64 ECHConfigList is full of `+` characters of its own.
fn strip_resolver(ech: &str) -> &str {
    for scheme in [
        "+https://",
        "+http://",
        "+quic://",
        "+h3://",
        "+dot://",
        "+tls://",
    ] {
        if let Some(at) = ech.find(scheme) {
            return &ech[..at];
        }
    }
    ech
}

fn looks_like_host(value: &str) -> bool {
    value.contains('.')
        && value.len() <= 253
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')
        })
}

fn query_over_udp(name: &str) -> Option<String> {
    let question = build_query(name);
    for resolver in RESOLVERS {
        let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else {
            continue;
        };
        let _ = sock.set_read_timeout(Some(QUERY_TIMEOUT));
        let _ = sock.set_write_timeout(Some(QUERY_TIMEOUT));
        if sock.send_to(&question, resolver).is_err() {
            continue;
        }
        // 4KB: an HTTPS record with hints and an ECHConfigList is ~200 bytes,
        // but a chained-CDN record with several answers is bigger than 512.
        let mut buf = [0u8; 4096];
        let Ok((len, _)) = sock.recv_from(&mut buf) else {
            continue;
        };
        if let Some(ech) = parse_ech_config(&buf[..len]) {
            log::debug!("ech: {} config resolved via {}", name, resolver);
            return Some(ech);
        }
    }
    log::debug!("ech: {} has no reachable resolver that answers HTTPS", name);
    None
}

fn build_query(name: &str) -> Vec<u8> {
    let mut q = Vec::with_capacity(64);
    q.extend_from_slice(&[0x12, 0x34]); // id
    q.extend_from_slice(&[0x01, 0x00]); // recursion desired
    q.extend_from_slice(&[0x00, 0x01]); // one question
    for _ in [0u8; 3] {
        q.extend_from_slice(&[0x00, 0x00]); // no answer / authority / additional
    }
    for label in name.split('.').filter(|l| !l.is_empty()) {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&[0x00, 65]); // HTTPS
    q.extend_from_slice(&[0x00, 0x01]); // IN
    // EDNS0, 4096-byte UDP payload: without it a resolver may truncate and drop
    // the ech parameter, which is the last one in the record.
    q.extend_from_slice(&[0x00, 0x29, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    q
}

/// Walk the answer section and return the first parameter that really is an
/// ECHConfigList. Key **numbers** are not relied on: the registry and the
/// records seen in the wild disagree about which one carries it, so the value's
/// own structure (2-byte length + ECH version) decides.
pub fn parse_ech_config(packet: &[u8]) -> Option<String> {
    if packet.len() < 12 {
        return None;
    }
    let ancount = u16::from_be_bytes([packet[6], packet[7]]) as usize;
    let mut p = 12;
    // The question is written out in full, so skipping it is just a label walk.
    p = skip_name(packet, p)? + 4;
    for _ in 0..ancount {
        p = skip_name(packet, p)?;
        if p + 10 > packet.len() {
            return None;
        }
        let rtype = u16::from_be_bytes([packet[p], packet[p + 1]]);
        let rdlen = u16::from_be_bytes([packet[p + 8], packet[p + 9]]) as usize;
        let rdata_start = p + 10;
        let rdata = packet.get(rdata_start..rdata_start + rdlen)?;
        p = rdata_start + rdlen;
        if rtype != 65 && rtype != 64 {
            continue;
        }
        if let Some(list) = ech_from_svcb(rdata) {
            return Some(list);
        }
    }
    None
}

/// `priority(2) target(name) SvcParam*`
fn ech_from_svcb(rdata: &[u8]) -> Option<String> {
    if rdata.len() < 3 {
        return None;
    }
    let mut o = 2; // skip priority
    loop {
        let len = *rdata.get(o)?;
        o += 1;
        if len == 0 {
            break;
        }
        if len >= 0xc0 {
            o += 1;
            break; // compressed target: nothing follows it but parameters
        }
        o += len as usize;
    }
    while o + 4 <= rdata.len() {
        let plen = u16::from_be_bytes([rdata[o + 2], rdata[o + 3]]) as usize;
        let value_start = o + 4;
        if value_start + plen > rdata.len() {
            break;
        }
        let value = &rdata[value_start..value_start + plen];
        if is_ech_config_list(value) {
            return Some(base64::engine::general_purpose::STANDARD.encode(value));
        }
        o = value_start + plen;
    }
    None
}

/// `ECHConfigList = OCTET_STRING(length) followed by ECHConfig{version}` where
/// the only versions in use are 0xFE0D (RFC 9461) and 0xFE08 (the old draft).
fn is_ech_config_list(v: &[u8]) -> bool {
    if v.len() < 6 {
        return false;
    }
    let total = u16::from_be_bytes([v[0], v[1]]) as usize;
    if total + 2 != v.len() {
        return false;
    }
    matches!(
        u16::from_be_bytes([v[2], v[3]]),
        0xfe0d | 0xfe08
    )
}

fn skip_name(packet: &[u8], mut p: usize) -> Option<usize> {
    let mut first_end = None;
    loop {
        let len = *packet.get(p)?;
        if len == 0 {
            p += 1;
            return Some(first_end.unwrap_or(p));
        }
        if len & 0xc0 == 0xc0 {
            let end = p + 2;
            if first_end.is_none() {
                first_end = Some(end);
            }
            // A pointer ends this name; everything after it is not ours to walk.
            return Some(first_end.unwrap_or(end));
        }
        p += 1 + len as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    /// A real answer from 223.5.5.5, captured 2026-09-25 off this network. The
    /// `ech` parameter is the third one in the record and is what mihomo needs.
    const ALIDNS_HTTPS: &str = "ab cd 81 80 00 01 00 01 00 00 00 00 0e 63 6c 6f 75 64 66 6c 61 72 65 2d 65 63 68 03 63 6f 6d 00 00 41 00 01 c0 0c 00 41 00 01 00 00 00 9a 00 88 00 01 00 00 01 00 06 02 68 33 02 68 32 00 04 00 08 68 12 0a 76 68 12 0b 76 00 05 00 47 00 45 fe 0d 00 41 fa 00 20 00 20 47 a3 ad 58 fe df 79 62 cb e1 96 d4 9d 52 e8 e6 fd 95 7c bc b8 3f 5a 39 a7 5a a7 7c 49 e6 43 38 00 04 00 01 00 01 00 12 63 6c 6f 75 64 66 6c 61 72 65 2d 65 63 68 2e 63 6f 6d 00 00 00 06 00 20 26 06 47 00 00 00 00 00 00 00 00 00 68 12 0a 76 26 06 47 00 00 00 00 00 00 00 00 00 68 12 0b 76";
    const KNOWN: &str = "AEX+DQBB+gAgACBHo61Y/t95YsvhltSdUujm/ZV8vLg/WjmnWqd8SeZDOAAEAAEAAQASY2xvdWRmbGFyZS1lY2guY29tAAA=";

    #[test]
    fn https_record_yields_the_ech_config_list() {
        assert_eq!(parse_ech_config(&hex(ALIDNS_HTTPS)).as_deref(), Some(KNOWN));
    }

    #[test]
    fn a_record_without_ech_gives_nothing_rather_than_garbage() {
        // Same packet with the ech parameter's version byte corrupted: the
        // caller must get None, so the relay block omits ech-opts instead of
        // handing mihomo a config it will fail every dial with.
        let mut p = hex(ALIDNS_HTTPS);
        let at = p.windows(2).position(|w| w == [0xfe, 0x0d]).unwrap();
        p[at] = 0xfe;
        p[at + 1] = 0x99;
        assert_eq!(parse_ech_config(&p), None);
    }

    #[test]
    fn link_parameter_is_read_both_ways() {
        // name + resolver url (what these panels send) → query the name
        assert_eq!(
            classify("cloudflare-ech.com+https://dns.alidns.com/dns-query"),
            Some(("cloudflare-ech.com", false))
        );
        // a bare base64 config → use it verbatim, no lookup
        assert_eq!(
            classify(KNOWN),
            Some((KNOWN, true))
        );
        assert_eq!(classify(""), None);
        assert_eq!(classify("  "), None);
    }

    #[test]
    fn inline_base64_config_is_used_as_is() {
        assert_eq!(config_for(KNOWN).as_deref(), Some(KNOWN));
    }

    /// 手工跑:`cargo test --lib ech::tests::live -- --ignored --nocapture`。
    /// 网络在跑,所以平时 ignore;它证明的是解析器之外的另一半 —— 真的能收到答复。
    #[test]
    #[ignore]
    fn live_resolver_actually_answers_the_https_query() {
        let got = config_for("cloudflare-ech.com+https://dns.alidns.com/dns-query");
        println!("live ECHConfig: {:?}", got.as_deref());
        let got = got.expect("no resolver answered the HTTPS record");
        let bytes = base64::engine::general_purpose::STANDARD.decode(&got).unwrap();
        assert!(
            is_ech_config_list(&bytes),
            "解析出来的不是 ECHConfigList: {got}"
        );
    }
}
