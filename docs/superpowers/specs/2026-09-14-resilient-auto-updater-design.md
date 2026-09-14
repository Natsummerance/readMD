# Design Spec: Resilient Multi-Tier Auto-Updater (2026-09-14)

## 1. Problem Statement & Background
In ReadMD versions up to v2.3.8, checking for updates in mainland China or constrained networks frequently results in "检查更新失败" (Update Check Failed). Root cause analysis identified three cascading points of failure:
1. **Mirror 403 Rejection**: `_release_check_urls()` prefixed mirrors (`ghfast.top`, `ghproxy.net`) with `https://api.github.com/...`. Since GitHub download mirrors only proxy raw files and release assets, querying REST APIs through them results in immediate `HTTP 403 Forbidden`.
2. **Direct API Block & Rate Limiting**: Direct calls to `api.github.com` suffer from domestic network interference (GFW/DNS poisoning/timeouts) and unauthenticated IP rate limits (60 req/hour). When direct connection fails and mirror fallback yields 403, the entire update check pipeline fails.
3. **HTTP 500 Swallowing Frontend Details**: In `readmd.py`, `_api_update_check()` responded with HTTP 500 whenever `res['ok']` was False. The frontend `fetch('/api/update/check')` skipped parsing the JSON body upon `!resp.ok`, discarding the specific `error_code` and showing an unhelpful generic error.

## 2. Goals & Non-Goals
### Goals
- **100% Reliable Version Detection**: Check for updates successfully in mainland China without VPN/proxy and under GitHub API rate-limited conditions.
- **Multi-Tier Fallback Cascade**:
  - Tier 1: Official GitHub REST API (`api.github.com`) with tight timeout (3.5s).
  - Tier 2: Official GitHub Web 302 redirect sniffing (`github.com/.../releases/latest` -> `Location` header `.../tag/vX.Y.Z`).
  - Tier 3: Mirror Web 302 redirect sniffing (`ghfast.top`, `ghproxy.net` -> `Location` header `.../tag/vX.Y.Z`).
  - Tier 4: `SHA256SUMS.txt` manifest parser for release asset reconstruction and hash pinning.
- **Resilient Multi-Mirror Download Failover**: If the primary download mirror fails or times out, seamlessly failover to the next candidate mirror.
- **Clean Communication Protocol**: Always return HTTP 200 with structured JSON (`ok`, `error_code`, `error`, `has_update`, `html_url`).
- **Cryptographic Hard Gate**: Retain strict SHA-256 integrity verification before any executable can be marked ready or executed.

### Non-Goals
- Requiring a self-hosted custom server or domain for updates.
- Altering the Inno Setup / portable executable application scripts on Windows.

## 3. Architecture & Detailed Design

### 3.1 Multi-Tier Check Cascade
```
                     check_update(current_version)
                                   |
                +------------------v------------------+
                | Tier 1: api.github.com              |  (Timeout 3.5s)
                | /repos/.../releases/latest          |
                +------------------+------------------+
                                   |
                       [Success]   |   [Fail / Timeout / 403]
                           +-------+-------+
                           |               |
                           v               v
                     Parse Release   +-----------------------------+
                     JSON Payload    | Tier 2: github.com Web 302  |
                                     | /.../releases/latest        |
                                     +--------------+--------------+
                                                    |
                                        [Success]   |   [Fail / Timeout]
                                            +-------+-------+
                                            |               |
                                            v               v
                                      Sniff Tag    +-----------------------------+
                                      From Header  | Tier 3: Mirror Web 302      |
                                                   | ghfast.top / ghproxy.net    |
                                                   +--------------+--------------+
                                                                  |
                                                      [Success]   |   [All Fail]
                                                          +-------+-------+
                                                          |               |
                                                          v               v
                                                    Sniff Tag       Return Error
                                                    From Header     (update_network_error)
                                                          |
                                                          v
                                      +-----------------------------------------+
                                      | Tier 4: Fetch SHA256SUMS.txt            |
                                      | (Direct -> Mirror 1 -> Mirror 2)        |
                                      +-------------------+---------------------+
                                                          |
                                                          v
                                      Reconstruct Assets List & Pin Hashes
                                                          |
                                                          v
                                      Match Platform Flavor & Compare SemVer
```

#### Sniffing Tag via HTTP 302
When requesting `https://github.com/Natsummerance/readMD/releases/latest` without following redirects:
- GitHub returns HTTP 302 with `Location: https://github.com/Natsummerance/readMD/releases/tag/v2.3.9`.
- Extract tag via regex: `r'/releases/tag/(v?[0-9]+\.[0-9]+[^\s/?#]*)'`.
- Mirror endpoints (`ghfast.top`, `ghproxy.net`) return `/https://github.com/.../releases/tag/v2.3.9` with identical tag info.

#### Reconstructing Assets from `SHA256SUMS.txt`
The manifest file `SHA256SUMS.txt` is fetched from:
1. `https://github.com/Natsummerance/readMD/releases/download/{tag}/SHA256SUMS.txt`
2. `https://ghfast.top/https://github.com/Natsummerance/readMD/releases/download/{tag}/SHA256SUMS.txt`
3. `https://ghproxy.net/https://github.com/Natsummerance/readMD/releases/download/{tag}/SHA256SUMS.txt`

Parsed lines `[hash] [filename]` yield the synthetic `assets` array:
```python
{
    'name': filename,
    'browser_download_url': f'https://github.com/{GITHUB_REPO}/releases/download/{tag}/{filename}',
    'expected_sha': hash_val,
    'size': 0
}
```
This is passed to `match_release_asset(assets, flavor)` with 100% contract parity.

### 3.2 Multi-Mirror Download Failover Engine
Candidate mirror wrappers:
```python
MIRROR_PREFIXES = [
    'https://ghfast.top/',
    'https://ghproxy.net/',
    'https://mirror.ghproxy.com/',
]
```
When `use_mirror=True`:
1. Build prioritized list of download URLs:
   `[prefix + official_download_url for prefix in MIRROR_PREFIXES] + [official_download_url]`
2. Try candidates in order. If candidate 1 encounters connection reset, HTTP 4xx/5xx or timeout during handshake:
   - Clean up partial file.
   - Proceed to candidate 2 immediately without user friction.
3. Stream file chunks to `.part` file.
4. Calculate SHA-256. If match is verified, execute `os.replace()` to target path and transition status to `ready`.

### 3.3 Server Endpoint & Frontend Error Contract
In `readmd.py`:
```python
def _api_update_check(self):
    try:
        from src.readmd_modules import updater
        res = updater.check_update(VERSION)
        self._send_json(200, res)  # Always 200 for clean contract
    except Exception as e:
        logging.exception('api_update_check failed')
        self._send_api_error(500, 'update_check_failed')
```

In `assets/js/features/updater.js`:
- Safe JSON parse: `const res = await resp.json().catch(() => null);`
- Provide actionable toast with link to manual GitHub Releases page when network fails.

## 4. Verification Plan
- Unit tests in `tests/test_updater.py` and `tests/test_updater_security.py`:
  - `test_check_update_302_web_sniffing`
  - `test_check_update_mirror_302_sniffing`
  - `test_check_update_manifest_reconstruction`
  - `test_download_multi_mirror_failover`
  - `test_updater_api_200_contract`
- Live integration verification in Python environment.
