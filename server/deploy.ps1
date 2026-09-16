Write-Host "========================================="
Write-Host " VPN Auth Server - Cloudflare Deployment "
Write-Host "========================================="
Write-Host ""

Set-Location -Path $PSScriptRoot

Write-Host "[1/4] Installing dependencies..."
npm install wrangler --no-save

Write-Host ""
Write-Host "[2/4] Authenticating with Cloudflare..."
Write-Host "      (A browser window will open. Please log in and authorize Wrangler)"
npx wrangler login

Write-Host ""
Write-Host "[3/4] Creating KV Namespace 'AUTH_DB'..."
$kvOutput = npx wrangler kv:namespace create AUTH_DB

Write-Host "KV Output: $kvOutput"

$kvId = ""
if ($kvOutput -match 'id = "([^"]+)"') {
    $kvId = $matches[1]
    Write-Host "Successfully extracted KV ID: $kvId"
} else {
    Write-Host "Failed to extract KV ID. It might already exist."
}

if ($kvId -ne "") {
    $tomlPath = "wrangler.toml"
    $tomlContent = Get-Content $tomlPath -Raw
    if ($tomlContent -notmatch "kv_namespaces") {
        Write-Host "Injecting KV Namespace into wrangler.toml..."
        $kvConfig = "`n[[kv_namespaces]]`nbinding = `"AUTH_DB`"`nid = `"$kvId`"`n"
        Add-Content -Path $tomlPath -Value $kvConfig
    } else {
        Write-Host "wrangler.toml already contains kv_namespaces binding. Skipping."
    }
}

Write-Host ""
Write-Host "[4/4] Deploying Worker to Cloudflare..."
npx wrangler deploy

Write-Host ""
Write-Host "========================================="
Write-Host " Deployment Complete! "
Write-Host "========================================="
Write-Host "Next Steps:"
Write-Host "1. Look at the published URL above (e.g., https://vpn-auth-server.<your-subdomain>.workers.dev)"
Write-Host "2. Go to Cloudflare Dashboard -> Workers & Pages -> KV"
Write-Host "3. Add your machine codes to AUTH_DB."
Write-Host "   Key: <Your Machine ID>"
Write-Host "   Value: {`"authorized`":true, `"display_text`":`"NarcissusAura VIP`"}"
Write-Host "========================================="
