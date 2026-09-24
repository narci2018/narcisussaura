//! Where VPNGate servers come from.
//!
//! One GitHub mirror used to be the whole answer: when it thinned or went down
//! the tab filled with the seven hard-coded Tsukuba seeds, and every node in the
//! list claimed to be alive because nothing ever checked. This module keeps the
//! mirror and adds the six official endpoints — `vpngate.net` and its
//! `opengw.net` mirrors, in CSV and HTML form.
//!
//! The official endpoints are GFW-blocked, so they are only ever fetched
//! through the probe lane's best relay — never through a public CORS wrapper.
//! The endpoints all mirror the same database, so the first one that answers is
//! enough; the rest are redundancy, not extra servers.
//!
//! What comes back is a fact, not a claim: `status: Unknown` with no latency
//! and no bandwidth until the liveness pass has actually dialled the server.

use std::collections::HashSet;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::managers::connection_manager::ConnectionManager;
use crate::managers::lane_core::{Lane, LANE_LIST_FETCH};
use crate::managers::node_manager::{GroupPrune, NodeManager};
use crate::managers::special_sources::SpecialSources;
use crate::models::{NodeStatus, ProtocolType, UnifiedNode};

/// Unblocked CDN mirror of the official dump. Reachable directly.
pub const MIRROR_JSON_URL: &str =
    "https://testingcf.jsdelivr.net/gh/GeorgeXie2333/vpngate-list-mirror@main/data/servers.json";

/// Official list endpoints, tried in this order, all through the relay.
pub const OFFICIAL_SOURCES: [&str; 6] = [
    "http://www.vpngate.net/api/iphone/",
    "https://www.vpngate.net/api/iphone/",
    "http://v-dot-pn-tok.opengw.net:33304/api/iphone/",
    "http://v-dot-pn-ams1.opengw.net:33304/api/iphone/",
    "http://v-dot-pn-ams1.opengw.net:33304/en/",
    "https://www.vpngate.net/en/",
];

/// A full liveness pass over this many servers already costs minutes on a
/// phone; the list is ordered by source, so the cap trims the tail, not the
/// servers that actually answer.
pub const MAX_SERVERS: usize = 300;

const LIST_FETCH_TIMEOUT: Duration = Duration::from_secs(12);

/// One server as some source described it. The port and ciphers are not here
/// on purpose — they live in the OpenVPN config, and that is what we dial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpngateEntry {
    pub host: String,
    pub country_code: String,
    pub country_name: String,
    pub ovpn_b64: String,
}

/// Split one CSV line, honouring the doubled-quote escaping the official file
/// uses — its `openvpn_configuration_files_base64` cell holds dozens of blobs
/// separated by commas inside a single quoted field.
pub fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if quoted && chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    quoted = !quoted;
                }
            }
            ',' if !quoted => fields.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    fields.push(current);
    fields
}

fn column_index(header: &[String], names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|want| header.iter().position(|h| h.trim().eq_ignore_ascii_case(want)))
}

fn cell(row: &[String], index: Option<usize>) -> &str {
    index
        .and_then(|i| row.get(i))
        .map(|s| s.trim())
        .unwrap_or("")
}

/// Ports that survive a censored network, in the order we prefer them. A
/// SoftEther server publishes one config per SSL port; the relay we hop
/// through is far more likely to carry 443 than 58084.
const PREFERRED_PORTS: [u16; 4] = [443, 80, 110, 22];

/// Choose one base64 OpenVPN config out of the comma-separated set a server
/// publishes. The winner is the first that dials on a well-known SSL port.
pub fn pick_ovpn_blob(blobs: &[&str]) -> Option<String> {
    let mut fallback: Option<String> = None;
    for blob in blobs.iter().map(|b| b.trim()).filter(|b| b.len() > 32) {
        let parsed = match SpecialSources::parse_openvpn_fields(blob) {
            Some(p) => p,
            None => continue,
        };
        if PREFERRED_PORTS.contains(&parsed.1) {
            return Some(blob.to_string());
        }
        if fallback.is_none() {
            fallback = Some(blob.to_string());
        }
    }
    fallback
}

/// Parse the official CSV dump. Header-driven on purpose: the column set has
/// changed across the years, and the names are the only stable thing in it.
struct Columns {
    ovpn: Option<usize>,
    cc: Option<usize>,
    name: Option<usize>,
    host: Option<usize>,
}

impl Columns {
    fn new(header: &[String]) -> Self {
        Self {
            ovpn: column_index(
                header,
                &[
                    "openvpn_configuration_files_base64",
                    "openvpn_config_base64",
                    "openvpn",
                ],
            ),
            cc: column_index(header, &["vpncc", "country_short", "country_code", "cc"]),
            name: column_index(header, &["vpnco", "country_long", "country_name"]),
            host: column_index(header, &["servername", "#hostname", "hostname", "host", "ip"]),
        }
    }
}

pub fn parse_vpngate_csv(text: &str) -> Vec<VpngateEntry> {
    let mut columns: Option<Columns> = None;
    let mut entries = Vec::new();

    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        if columns.is_none() {
            if line.to_ascii_lowercase().contains("hostname") {
                columns = Some(Columns::new(&split_csv_line(line)));
            }
            continue;
        }
        let columns = columns.as_ref().unwrap();
        let row = split_csv_line(line);

        let blobs: Vec<&str> = cell(&row, columns.ovpn).split(',').collect();
        let ovpn = match pick_ovpn_blob(&blobs) {
            Some(o) => o,
            None => continue,
        };

        let code = cell(&row, columns.cc).to_ascii_uppercase();
        entries.push(VpngateEntry {
            host: cell(&row, columns.host).to_string(),
            country_code: if code.is_empty() { "??".to_string() } else { code.clone() },
            country_name: {
                let name = cell(&row, columns.name);
                if name.is_empty() { code } else { name.to_string() }
            },
            ovpn_b64: ovpn,
        });

        if entries.len() >= MAX_SERVERS {
            break;
        }
    }

    if columns.is_none() {
        log::warn!("vpngate CSV has no `hostname` header, nothing parsed");
    }
    entries
}

/// The `/en/` pages carry the same CSV inside a `<pre>` block.
pub fn extract_csv_from_html(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<pre")?;
    let open = html[start..].find('>')? + start + 1;
    let end_rel = html[open..].to_ascii_lowercase().find("</pre>")?;
    let block = &html[open..open + end_rel];
    if block.to_ascii_lowercase().contains("hostname") {
        Some(block.to_string())
    } else {
        None
    }
}

/// The CDN mirror's own JSON shape.
pub fn parse_mirror_json(body: &str) -> Vec<VpngateEntry> {
    let val: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("vpngate mirror JSON unparseable: {}", e);
            return Vec::new();
        }
    };
    let servers = match val.get("servers").and_then(|s| s.as_array()) {
        Some(a) => a.clone(),
        None => return Vec::new(),
    };

    servers
        .iter()
        .filter_map(|s| {
            let text = |keys: &[&str]| -> String {
                keys.iter()
                    .find_map(|k| s.get(*k).and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string()
            };
            let config = text(&["openvpn_config_base64", "openvpn_configuration_files_base64"]);
            let blobs: Vec<&str> = config.split(',').collect();
            let ovpn = pick_ovpn_blob(&blobs)?;
            let code = text(&["country_code", "country_short"]).to_ascii_uppercase();
            let name = text(&["country_name", "country_long"]);
            Some(VpngateEntry {
                host: text(&["ip", "hostname"]),
                country_code: if code.is_empty() { "??".to_string() } else { code.clone() },
                country_name: if name.is_empty() { code } else { name },
                ovpn_b64: ovpn,
            })
        })
        .collect()
}

/// Merge every source into node records, deduplicated on the server we dial.
///
/// The id carries the port because hosts do repeat across SoftEther's SSL
/// ports, and an id without it made two endpoints of one machine overwrite
/// each other in the store.
pub fn merge_nodes(entries: &[VpngateEntry]) -> Vec<UnifiedNode> {
    let mut seen: HashSet<(String, u16)> = HashSet::new();
    let mut nodes = Vec::new();

    for entry in entries {
        let (host, port, proto, cipher, auth, ca, cert, key) =
            match SpecialSources::parse_openvpn_fields(&entry.ovpn_b64) {
                Some(f) => f,
                None => continue,
            };
        if !seen.insert((host.clone(), port)) {
            continue;
        }
        if nodes.len() >= MAX_SERVERS {
            break;
        }

        nodes.push(UnifiedNode {
            id: format!("vpngate-{}-{}", host.replace('.', "-"), port),
            name: format!("VPNGate [{}] {}·{}", entry.country_code, host, port),
            protocol: ProtocolType::Openvpn,
            port,
            address: host.clone(),
            country_code: entry.country_code.clone(),
            country_name: entry.country_name.clone(),
            city: "SoftEther Relay".to_string(),
            group: "VPNGate".to_string(),
            tags: vec![
                "VPNGate".to_string(),
                "SoftEther".to_string(),
                "OpenVPN".to_string(),
            ],
            favorite: false,
            // Nothing above is a measurement. The liveness pass owns these
            // three fields, and until it runs the node is Unknown.
            latency_ms: None,
            speed_bps: None,
            last_checked: None,
            status: NodeStatus::Unknown,
            config: json!({
                "proto": proto,
                "cipher": cipher,
                "auth": auth,
                "ca": ca,
                "cert": cert,
                "key": key,
                "openvpn_config_base64": entry.ovpn_b64,
            }),
        });
    }

    nodes
}

/// The mirror is reachable without help; the official endpoints are not, so
/// they ride the lane's relay hop. Returns the raw documents that succeeded.
async fn fetch_official_through_relay(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
) -> Vec<(String, String)> {
    let relay = match node_manager.get_best_relay_node(preferred_id) {
        Some(r) => r,
        None => {
            log::warn!("vpngate official sources skipped: no relay candidate available at all");
            return Vec::new();
        }
    };
    let relay_yaml = match ConnectionManager::format_mihomo_relay_proxy(&relay, "relay") {
        Some(y) => y,
        None => {
            log::warn!(
                "vpngate official sources skipped: relay {} cannot be expressed in mihomo",
                relay.name
            );
            return Vec::new();
        }
    };

    let binary = match conn.locate_binary(app, "mihomo") {
        Ok(b) => b,
        Err(e) => {
            log::warn!("vpngate official sources skipped: {}", e);
            return Vec::new();
        }
    };
    let dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("vpngate official sources skipped: app_data_dir {}", e);
            return Vec::new();
        }
    };

    let lane = match tokio::task::spawn_blocking(move || {
        Lane::start(LANE_LIST_FETCH, &dir, &binary, Some(relay_yaml.as_str()), &[])
    })
    .await
    {
        Ok(Ok(lane)) => lane,
        Ok(Err(e)) => {
            log::warn!("vpngate list lane did not start: {}", e);
            return Vec::new();
        }
        Err(e) => {
            log::warn!("vpngate list lane join failed: {}", e);
            return Vec::new();
        }
    };

    if let Err(e) = lane.select("relay").await {
        log::warn!("vpngate list lane cannot target the relay: {}", e);
        drop(lane);
        return Vec::new();
    }

    let mut documents = Vec::new();
    // One endpoint is enough: every official list is the same database, so
    // pulling six of them only multiplies the wait for the tab.
    for url in OFFICIAL_SOURCES.iter() {
        match lane.fetch_through(url, LIST_FETCH_TIMEOUT).await {
            Ok(body) => {
                log::info!("vpngate official list served by {} ({} bytes)", url, body.len());
                documents.push((url.to_string(), body));
                break;
            }
            Err(e) => log::warn!("vpngate official source {} failed: {}", url, e),
        }
    }
    drop(lane);
    documents
}

/// Parse whichever document came back — the API answers with CSV, the `/en/`
/// pages wrap the same table in `<pre>`.
pub fn parse_official_document(body: &str) -> Vec<VpngateEntry> {
    if let Some(csv) = extract_csv_from_html(body) {
        let entries = parse_vpngate_csv(&csv);
        if !entries.is_empty() {
            return entries;
        }
    }
    parse_vpngate_csv(body)
}

/// What one round of collection found.
pub struct Collected {
    pub nodes: Vec<UnifiedNode>,
    /// True when an official dump answered. That dump covers every registered
    /// server, so the group can be rebuilt from scratch; the CDN mirror is only
    /// a slice of it, and must never delete the rest.
    pub full_dump: bool,
}

/// Collect every source and merge it into node records.
pub async fn collect_nodes(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
) -> Collected {
    let mut entries: Vec<VpngateEntry> = Vec::new();
    let mut full_dump = false;

    match crate::managers::url_fallback::fetch_with_smart_fallback(MIRROR_JSON_URL, None, 8, None)
        .await
    {
        Ok((body, _)) => {
            let mirror = parse_mirror_json(&body);
            log::info!("vpngate mirror: {} servers", mirror.len());
            entries.extend(mirror);
        }
        Err(e) => log::warn!("vpngate mirror unreachable: {}", e),
    }

    for (url, body) in fetch_official_through_relay(app, node_manager, conn, preferred_id).await {
        let official = parse_official_document(&body);
        log::info!("vpngate official {}: {} servers", url, official.len());
        full_dump = full_dump || !official.is_empty();
        entries.extend(official);
    }

    let nodes = merge_nodes(&entries);
    log::info!(
        "vpngate: {} distinct servers after merge/dedupe ({})",
        nodes.len(),
        if full_dump { "full official dump" } else { "partial sources only" }
    );
    Collected { nodes, full_dump }
}

/// Fetch, merge, publish. The VPNGate group in the store gains everything the
/// sources say, and — when a full official dump answered — forgets what the
/// project no longer lists. With nothing at all to show, the built-in seeds
/// keep the tab usable offline.
pub async fn sync(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
) -> Result<Vec<UnifiedNode>, String> {
    let collected = collect_nodes(app, node_manager, conn, preferred_id).await;
    let mut nodes = collected.nodes;
    if nodes.is_empty() {
        log::warn!("vpngate: no source answered, falling back to the built-in seeds");
        nodes = SpecialSources::build_static_vpngate_nodes();
    }
    let nodes = node_manager.replace_group(
        "VPNGate",
        nodes,
        if collected.full_dump {
            GroupPrune::DropMissing
        } else {
            GroupPrune::KeepMissing
        },
    )?;
    // The tab shows the list as soon as it lands; the verdicts arrive later,
    // one batch at a time, from the liveness pass.
    let _ = app.emit("nodes:updated", nodes.len());
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn ovpn(host: &str, port: u16) -> String {
        let cfg = format!(
            "client\nremote {} {}\nproto tcp\nresolv-retry infinite\ncipher AES-128-CBC\nauth SHA1\n<ca>\n-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n</ca>\n",
            host, port
        );
        base64::engine::general_purpose::STANDARD.encode(cfg)
    }

    /// Header shaped like the documented vpngate iPhone API dump. The column
    /// set here is a hand-written sample, NOT a captured response — the real
    /// site is unreachable from the development network, so the parser is
    /// written to survive unknown columns instead of trusting this list.
    fn csv_header() -> String {
        "#hostname,servername,port_tcp_http,vpncc,vpnco,speed_dl,openvpn_configuration_files_base64".to_string()
    }

    #[test]
    fn csv_is_read_by_column_name_not_position() {
        let line = format!(
            "srv.example.com,srv.example.com,443,US,United States,1234,{}",
            ovpn("srv.example.com", 443)
        );
        let shuffled = "#hostname,openvpn_configuration_files_base64,vpncc,speed_dl,servername,vpnco,port_tcp_http".to_string();
        let shuffled_line = format!(
            "srv.example.com,{ovpn},US,1234,srv.example.com,United States,443",
            ovpn = ovpn("srv.example.com", 443)
        );

        let direct = parse_vpngate_csv(&format!("{}\n{}\n", csv_header(), line));
        let reordered = parse_vpngate_csv(&format!("{}\n{}\n", shuffled, shuffled_line));

        assert_eq!(direct.len(), 1);
        assert_eq!(direct, reordered, "column order must not matter");
        assert_eq!(direct[0].country_code, "US");
        assert_eq!(direct[0].country_name, "United States");
    }

    #[test]
    fn quoted_cell_with_comma_separated_blobs_survives_splitting() {
        let blobs = format!("{},{}", ovpn("a.example.com", 58084), ovpn("a.example.com", 443));
        let line = format!("a.example.com,a.example.com,443,JP,Japan,10,\"{}\"", blobs);
        let entries = parse_vpngate_csv(&format!("{}\n{}\n", csv_header(), line));

        assert_eq!(entries.len(), 1);
        // the preferred port wins even though it is second in the cell
        let (_, port, _, _, _, _, _, _) =
            SpecialSources::parse_openvpn_fields(&entries[0].ovpn_b64).unwrap();
        assert_eq!(port, 443);
    }

    #[test]
    fn unknown_and_unparsable_rows_are_dropped_not_guessed() {
        let text = format!(
            "{}\nno-config.example.com,no-config.example.com,443,DE,Germany,10,\nbroken,,,,,,not-base64!!\n",
            csv_header()
        );
        assert!(parse_vpngate_csv(&text).is_empty());
    }

    #[test]
    fn html_pre_block_feeds_the_same_parser() {
        let body = format!(
            "<html><body><pre>{}\n{}\n</pre></body></html>",
            csv_header(),
            format!(
                "b.example.com,b.example.com,80,HK,Hong Kong,7,{}",
                ovpn("b.example.com", 80)
            )
        );
        let csv = extract_csv_from_html(&body).expect("<pre> block");
        let entries = parse_vpngate_csv(&csv);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].country_code, "HK");
    }

    #[test]
    fn a_pre_block_without_a_table_is_not_treated_as_the_list() {
        assert!(extract_csv_from_html("<html><pre>Welcome to VPNGate</pre></html>").is_none());
        assert!(extract_csv_from_html("<html><body>no pre at all</body></html>").is_none());
    }

    #[test]
    fn merged_nodes_carry_the_port_and_dedupe_by_dialled_endpoint() {
        let entries = vec![
            VpngateEntry {
                host: "c.example.com".to_string(),
                country_code: "SG".to_string(),
                country_name: "Singapore".to_string(),
                ovpn_b64: ovpn("c.example.com", 443),
            },
            VpngateEntry {
                host: "c.example.com".to_string(),
                country_code: "SG".to_string(),
                country_name: "Singapore".to_string(),
                ovpn_b64: ovpn("c.example.com", 443),
            },
            VpngateEntry {
                host: "c.example.com".to_string(),
                country_code: "SG".to_string(),
                country_name: "Singapore".to_string(),
                ovpn_b64: ovpn("c.example.com", 80),
            },
        ];
        let nodes = merge_nodes(&entries);

        assert_eq!(nodes.len(), 2, "same host on two ports is two endpoints");
        assert_eq!(nodes[0].id, "vpngate-c-example-com-443");
        assert_eq!(nodes[1].id, "vpngate-c-example-com-80");
        assert_eq!(nodes[0].port, 443);
        assert_eq!(nodes[1].port, 80);
    }

    #[test]
    fn merged_nodes_claim_nothing_until_something_dials_them() {
        let node = merge_nodes(&[VpngateEntry {
            host: "d.example.com".to_string(),
            country_code: "TW".to_string(),
            country_name: "Taiwan".to_string(),
            ovpn_b64: ovpn("d.example.com", 443),
        }])
        .remove(0);

        assert_eq!(node.status, NodeStatus::Unknown);
        assert_eq!(node.latency_ms, None);
        assert_eq!(node.speed_bps, None);
        assert_eq!(node.group, "VPNGate");
        assert_eq!(node.protocol, ProtocolType::Openvpn);
    }

    #[test]
    fn mirror_json_and_csv_describe_the_same_server_the_same_way() {
        let blob = ovpn("e.example.com", 443);
        let json = format!(
            "{{\"servers\":[{{\"ip\":\"e.example.com\",\"country_code\":\"us\",\"country_name\":\"United States\",\"openvpn_config_base64\":\"{}\"}}]}}",
            blob
        );
        let from_json = parse_mirror_json(&json);
        let from_csv = parse_vpngate_csv(&format!(
            "{}\ne.example.com,e.example.com,443,US,United States,1,{}\n",
            csv_header(),
            blob
        ));

        assert_eq!(from_json.len(), 1);
        assert_eq!(from_json[0].country_code, "US");
        let merged = merge_nodes(&[from_json[0].clone(), from_csv[0].clone()]);
        assert_eq!(merged.len(), 1, "dedupe is what makes two sources one list");
    }

    #[test]
    fn merge_stops_at_the_cap_so_a_runaway_source_cannot_freeze_the_tab() {
        let entries: Vec<VpngateEntry> = (0..(MAX_SERVERS + 500))
            .map(|i| VpngateEntry {
                host: format!("h{}.example.com", i),
                country_code: "US".to_string(),
                country_name: "United States".to_string(),
                ovpn_b64: ovpn(&format!("h{}.example.com", i), 443),
            })
            .collect();
        assert_eq!(merge_nodes(&entries).len(), MAX_SERVERS);
    }
}
