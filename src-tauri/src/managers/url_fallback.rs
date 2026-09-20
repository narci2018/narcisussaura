use regex::Regex;
use std::time::Duration;

/// Generates an intelligent, prioritized list of candidate URLs for fault tolerance.
///
/// Endpoints hosted on GitHub or jsDelivr (e.g. `cdn.jsdelivr.net` or `raw.githubusercontent.com`)
/// are frequently blocked by GFW (SNI RST injection / DNS poisoning) in mainland China.
///
/// This resolver extracts the target repository and path, and generates high-availability mirrors:
/// 1. `https://testingcf.jsdelivr.net/gh/...` (Unblocked Cloudflare edge)
/// 2. `https://ghproxy.net/https://raw.githubusercontent.com/...` (High availability GitHub accelerator)
/// 3. `https://gh-proxy.com/https://raw.githubusercontent.com/...` (Backup GitHub accelerator)
/// 4. `https://fastly.jsdelivr.net/gh/...` (Fastly CDN mirror)
/// 5. `https://gcore.jsdelivr.net/gh/...` (Gcore CDN mirror)
/// 6. Direct raw GitHub URL
/// 7. Original URL (if not already included)
pub fn generate_fallback_urls(original_url: &str) -> Vec<String> {
    let original = original_url.trim();
    if original.is_empty() {
        return vec![];
    }

    let mut candidates = Vec::new();
    let mut parsed: Option<(String, String, String, String)> = None; // (user, repo, branch, path)

    // Pattern 1: jsdelivr github: https://{subdomain}.jsdelivr.net/gh/:user/:repo(@:branch)?/:path
    if original.contains("jsdelivr.net/gh/") {
        let re = Regex::new(r"jsdelivr\.net/gh/([^/@]+)/([^/@]+)(?:@([^/]+))?/(.+)").unwrap();
        if let Some(caps) = re.captures(original) {
            let user = caps.get(1).map_or("", |m| m.as_str()).to_string();
            let repo = caps.get(2).map_or("", |m| m.as_str()).to_string();
            let branch = caps.get(3).map_or("main", |m| m.as_str()).to_string();
            let path = caps.get(4).map_or("", |m| m.as_str()).to_string();
            parsed = Some((user, repo, branch, path));
        }
    } else if original.contains("raw.githubusercontent.com/") {
        // Pattern 2: raw.githubusercontent.com/:user/:repo/:branch/:path
        let re = Regex::new(r"raw\.githubusercontent\.com/([^/]+)/([^/]+)/([^/]+)/(.+)").unwrap();
        if let Some(caps) = re.captures(original) {
            let user = caps.get(1).map_or("", |m| m.as_str()).to_string();
            let repo = caps.get(2).map_or("", |m| m.as_str()).to_string();
            let branch = caps.get(3).map_or("main", |m| m.as_str()).to_string();
            let path = caps.get(4).map_or("", |m| m.as_str()).to_string();
            parsed = Some((user, repo, branch, path));
        }
    } else if original.contains("github.com/") && (original.contains("/raw/") || original.contains("/blob/")) {
        // Pattern 3: github.com/:user/:repo/(raw|blob)/:branch/:path
        let re = Regex::new(r"github\.com/([^/]+)/([^/]+)/(?:raw|blob)/([^/]+)/(.+)").unwrap();
        if let Some(caps) = re.captures(original) {
            let user = caps.get(1).map_or("", |m| m.as_str()).to_string();
            let repo = caps.get(2).map_or("", |m| m.as_str()).to_string();
            let branch = caps.get(3).map_or("main", |m| m.as_str()).to_string();
            let path = caps.get(4).map_or("", |m| m.as_str()).to_string();
            parsed = Some((user, repo, branch, path));
        }
    }

    if let Some((user, repo, branch, path)) = parsed {
        // High-priority working mirrors in mainland China
        candidates.push(format!("https://testingcf.jsdelivr.net/gh/{}/{}@{}/{}", user, repo, branch, path));
        candidates.push(format!("https://ghproxy.net/https://raw.githubusercontent.com/{}/{}/{}/{}", user, repo, branch, path));
        candidates.push(format!("https://gh-proxy.com/https://raw.githubusercontent.com/{}/{}/{}/{}", user, repo, branch, path));
        candidates.push(format!("https://fastly.jsdelivr.net/gh/{}/{}@{}/{}", user, repo, branch, path));
        candidates.push(format!("https://gcore.jsdelivr.net/gh/{}/{}@{}/{}", user, repo, branch, path));
        candidates.push(format!("https://raw.githubusercontent.com/{}/{}/{}/{}", user, repo, branch, path));
        candidates.push(format!("https://cdn.jsdelivr.net/gh/{}/{}@{}/{}", user, repo, branch, path));
    } else {
        // Generic fallback replacement for other URLs containing jsdelivr or github
        if original.contains("cdn.jsdelivr.net") {
            candidates.push(original.replace("cdn.jsdelivr.net", "testingcf.jsdelivr.net"));
            candidates.push(original.replace("cdn.jsdelivr.net", "fastly.jsdelivr.net"));
            candidates.push(original.replace("cdn.jsdelivr.net", "gcore.jsdelivr.net"));
        }
        if original.contains("raw.githubusercontent.com") {
            candidates.push(format!("https://ghproxy.net/{}", original));
            candidates.push(format!("https://gh-proxy.com/{}", original));
        }
    }

    // Always include the original input URL
    if !candidates.iter().any(|c| c == original) {
        candidates.push(original.to_string());
    }

    // Deduplicate preserving order
    let mut unique = Vec::new();
    for c in candidates {
        if !unique.contains(&c) {
            unique.push(c);
        }
    }

    unique
}

/// Fetches remote content with smart fault-tolerant fallback across mirrors.
/// Tries each candidate mirror with timeout and optional proxy, logging progress.
pub async fn fetch_with_smart_fallback(
    original_url: &str,
    proxy_url: Option<&str>,
    timeout_per_try_secs: u64,
    custom_user_agent: Option<&str>,
) -> anyhow::Result<(String, reqwest::header::HeaderMap)> {
    let urls = generate_fallback_urls(original_url);
    let mut last_err = anyhow::anyhow!("No candidate URLs to attempt for {}", original_url);

    let ua = custom_user_agent.unwrap_or("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 NarcissusAura/1.0.0");

    for candidate_url in &urls {
        log::info!("SmartFetch: attempting mirror '{}'...", candidate_url);

        let mut client_builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_per_try_secs));

        if let Some(p) = proxy_url {
            if let Ok(proxy) = reqwest::Proxy::all(p) {
                client_builder = client_builder.proxy(proxy);
            }
        }

        let client = match client_builder.build() {
            Ok(c) => c,
            Err(e) => {
                last_err = e.into();
                continue;
            }
        };

        let resp = match client
            .get(candidate_url)
            .header("User-Agent", ua)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::warn!("SmartFetch: mirror '{}' failed: {}", candidate_url, e);
                last_err = e.into();
                continue;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            log::warn!("SmartFetch: mirror '{}' returned HTTP {}", candidate_url, status);
            last_err = anyhow::anyhow!("HTTP request failed with status: {}", status);
            continue;
        }

        let headers = resp.headers().clone();
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                log::warn!("SmartFetch: mirror '{}' reading body failed: {}", candidate_url, e);
                last_err = e.into();
                continue;
            }
        };

        log::info!("SmartFetch: successfully fetched from '{}' (size: {} bytes)", candidate_url, body.len());
        return Ok((body, headers));
    }

    Err(last_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_fallback_urls_jsdelivr() {
        let input = "https://cdn.jsdelivr.net/gh/narci2018/freesubplus@main/output/v2ray.txt";
        let fallbacks = generate_fallback_urls(input);
        assert!(fallbacks.contains(&"https://testingcf.jsdelivr.net/gh/narci2018/freesubplus@main/output/v2ray.txt".to_string()));
        assert!(fallbacks.contains(&"https://ghproxy.net/https://raw.githubusercontent.com/narci2018/freesubplus/main/output/v2ray.txt".to_string()));
        assert!(fallbacks.contains(&"https://fastly.jsdelivr.net/gh/narci2018/freesubplus@main/output/v2ray.txt".to_string()));
        assert!(fallbacks.contains(&"https://gcore.jsdelivr.net/gh/narci2018/freesubplus@main/output/v2ray.txt".to_string()));
    }

    #[test]
    fn test_generate_fallback_urls_raw_github() {
        let input = "https://raw.githubusercontent.com/narci2018/freesubplus/main/output/residential_nodes.json";
        let fallbacks = generate_fallback_urls(input);
        assert!(fallbacks.contains(&"https://testingcf.jsdelivr.net/gh/narci2018/freesubplus@main/output/residential_nodes.json".to_string()));
        assert!(fallbacks.contains(&"https://ghproxy.net/https://raw.githubusercontent.com/narci2018/freesubplus/main/output/residential_nodes.json".to_string()));
    }
}

