use std::fs;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD, engine::general_purpose::URL_SAFE as BASE64_URL_SAFE};

fn detect_country_code_from_name(name: &str) -> String {
    let name_upper = name.to_uppercase();
    if name.contains("香港") || name_upper.contains("HK") || name_upper.contains("HONG KONG") {
        "HK".to_string()
    } else if name.contains("台湾") || name.contains("台灣") || name_upper.contains("TW") || name_upper.contains("TAIWAN") {
        "TW".to_string()
    } else if name.contains("日本") || name_upper.contains("JP") || name_upper.contains("JAPAN") || name_upper.contains("TOKYO") {
        "JP".to_string()
    } else if name.contains("美国") || name.contains("美國") || name_upper.contains("US") || name_upper.contains("UNITED STATES") || name_upper.contains("USA") {
        "US".to_string()
    } else if name.contains("新加坡") || name.contains("狮城") || name_upper.contains("SG") || name_upper.contains("SINGAPORE") {
        "SG".to_string()
    } else if name.contains("韩国") || name_upper.contains("KR") || name_upper.contains("KOREA") {
        "KR".to_string()
    } else if name.contains("英国") || name_upper.contains("UK") || name_upper.contains("GB") || name_upper.contains("BRITAIN") {
        "GB".to_string()
    } else if name.contains("德国") || name_upper.contains("DE") || name_upper.contains("GERMANY") {
        "DE".to_string()
    } else {
        "".to_string()
    }
}

fn main() {
    let body = reqwest::blocking::get("https://cdn.jsdelivr.net/gh/narci2018/freesubplus@main/output/v2ray.txt").unwrap().text().unwrap();
    let decoded = BASE64_STANDARD.decode(body.trim().as_bytes()).or_else(|_| BASE64_URL_SAFE.decode(body.trim().as_bytes())).unwrap();
    let text = String::from_utf8(decoded).unwrap();
    
    for (i, line) in text.lines().enumerate().take(5) {
        if line.starts_with("vless://") {
            let parsed = url::Url::parse(line).unwrap();
            let name = parsed.fragment().map(|f| urlencoding::decode(f).unwrap_or(f.into()).to_string()).unwrap_or_else(|| "Unknown".to_string());
            let code = detect_country_code_from_name(&name);
            println!("Node {}: name='{}', code='{}'", i, name, code);
        }
    }
}
