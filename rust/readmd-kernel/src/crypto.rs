// -*- coding: utf-8 -*-
//! ReadMD 凭据加密模块 — **Fernet 兼容**认证加密。
//!
//! 这是 `src/readmd_modules/crypto.py` 的 1:1 对等实现：Python 用
//! `cryptography.fernet.Fernet`（`crypto.py:24`）加密落盘的 API Key，密钥文件
//! 是 `DATA_DIR/encryption.key` 里一行 44 字符的 base64url 文本
//! （`crypto.py:36-56`）。因此 Rust 侧必须实现同一套令牌格式，否则 Python 写出
//! 的 `enc:` 值内核永远读不出来（反之亦然）。
//!
//! 令牌格式（与 `crypto.py:67-69` / `crypto.py:89` 使用的库完全一致）：
//!
//! ```text
//! base64url( 0x80 || timestamp_be_u64 || iv[16]
//!            || AES-128-CBC(key[16:32], PKCS7(plaintext))
//!            || HMAC-SHA256(key[0:16], 前面所有字节)[32] )   # 标准 base64url，带 '=' 填充
//! ```
//!
//! 语义对齐点（逐行镜像 Python）：
//! * 空串进出都是空串（`crypto.py:61-62`, `crypto.py:77-78`）。
//! * 写入返回 `'enc:' + token`（`crypto.py:69`）；读取时缺少 `enc:` 前缀说明是
//!   历史明文，**拒绝读取**（`crypto.py:79-81`）。
//! * `InvalidToken`（签名不符／版本不对／长度不合法）与任何其它解密异常都只记
//!   日志并返回空串（`crypto.py:91-96`）；加密侧的异常则必须抛出、拒绝落盘
//!   （`crypto.py:70-72`，fail closed）。
//! * 已存在的密钥文件优先，且只 `.strip()` 使用、绝不覆写（`crypto.py:44-46`）；
//!   不存在时才 `Fernet.generate_key()` 并写入，非 Windows 才 chmod 0600
//!   （`crypto.py:47-55`）。
//!
//! 依赖策略：AES 原语用已经在 `Cargo.lock` 中的 `aes`/`cbc`（`lopdf` 的传递依
//! 赖，离线可编译），HMAC-SHA256 直接在已有 `sha2` 上手写——`hmac` crate 不在
//! 依赖树里，为一个 25 行的构造新增 crate 不值得。
//!
//! 旧 AES-256-GCM 路径已删除而非保留为回退：`crypto.rs` 的公开函数在 kernel 内
//! 没有任何调用点（`grep -rn "crypto::" src` 为空），Rust 从未写过一份 GCM 密文，
//! 留着只会让 28 字节的 GCM 载荷和 XOR 回退把垃圾当成"密钥"返回。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use aes::Aes128;
use base64::Engine;
use serde_json::Map;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::paths;

const SERVICE_NAME: &str = "ReadMD";

/// `Fernet.generate_key()` = `urlsafe_b64encode(os.urandom(32))`: 32 raw bytes
/// split into the 16-byte signing key and the 16-byte AES-128 key.
const FERNET_KEY_RAW_LEN: usize = 32;
const SUBKEY_LEN: usize = 16;
const BLOCK_LEN: usize = 16;
const IV_LEN: usize = 16;
const MAC_LEN: usize = 32;
/// version (1) + timestamp (8) + IV.
const TOKEN_HEADER_LEN: usize = 1 + 8 + IV_LEN;
/// The shortest legal token: header + one PKCS7 block + MAC.
const TOKEN_MIN_LEN: usize = TOKEN_HEADER_LEN + BLOCK_LEN + MAC_LEN;
const FERNET_VERSION: u8 = 0x80;
/// SHA-256 block size; HMAC keys longer than this are hashed first.
const HMAC_BLOCK_LEN: usize = 64;

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128CbcDec = cbc::Decryptor<Aes128>;

// ------------------------------------------------------------------- base64

/// `base64.urlsafe_b64decode` as CPython actually behaves: both alphabets are
/// accepted, stray whitespace is ignored, but missing padding is an error.
/// Verified against the installed library — `' '+token`, `token+'\n'` and the
/// `+/` spelling all decrypt, `token.rstrip('=')` raises `InvalidToken`.
fn b64url_decode_lenient(text: &str) -> Option<Vec<u8>> {
    let normalized: String = text
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| match c {
            '+' => '-',
            '/' => '_',
            other => other,
        })
        .collect();
    base64::engine::general_purpose::URL_SAFE
        .decode(normalized.as_bytes())
        .ok()
}

fn b64url_encode(bytes: &[u8]) -> String {
    // Standard padding: Python writes `urlsafe_b64encode`, and its decoder
    // rejects a token whose `=` padding was dropped.
    base64::engine::general_purpose::URL_SAFE.encode(bytes)
}

// -------------------------------------------------------- HMAC-SHA256 (hand)

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&out);
    buf
}

/// RFC 2104 HMAC-SHA256 over the already-declared `sha2` dependency.
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0u8; HMAC_BLOCK_LEN];
    if key.len() > HMAC_BLOCK_LEN {
        block[..32].copy_from_slice(&sha256(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }

    let mut inner_key = [0u8; HMAC_BLOCK_LEN];
    let mut outer_key = [0u8; HMAC_BLOCK_LEN];
    for i in 0..HMAC_BLOCK_LEN {
        inner_key[i] = block[i] ^ 0x36;
        outer_key[i] = block[i] ^ 0x5c;
    }

    let inner = {
        let mut h = Sha256::new();
        h.update(inner_key);
        h.update(message);
        h.finalize()
    };
    let mut h = Sha256::new();
    h.update(outer_key);
    h.update(inner);
    let out = h.finalize();
    let mut mac = [0u8; 32];
    mac.copy_from_slice(&out);
    mac
}

/// `hmac.compare_digest` — `cryptography` uses the constant-time variant, and a
/// MAC check that leaks its first matching byte is not a MAC check.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

// -------------------------------------------------------------------- fernet

/// A parsed Fernet key: `signing | encryption`, the split `cryptography` does in
/// `Fernet.__init__`.
#[derive(Clone)]
struct FernetKey {
    signing: [u8; SUBKEY_LEN],
    encryption: [u8; SUBKEY_LEN],
}

impl FernetKey {
    /// `Fernet(key)` raising `ValueError` is what a bad key file means here.
    fn from_base64(raw: &[u8]) -> Option<FernetKey> {
        let text = std::str::from_utf8(raw).ok()?;
        let bytes = b64url_decode_lenient(text)?;
        if bytes.len() != FERNET_KEY_RAW_LEN {
            return None;
        }
        let mut key = FernetKey {
            signing: [0u8; SUBKEY_LEN],
            encryption: [0u8; SUBKEY_LEN],
        };
        key.signing.copy_from_slice(&bytes[..SUBKEY_LEN]);
        key.encryption
            .copy_from_slice(&bytes[SUBKEY_LEN..FERNET_KEY_RAW_LEN]);
        Some(key)
    }

    fn generate_base64() -> Result<Vec<u8>> {
        let mut raw = [0u8; FERNET_KEY_RAW_LEN];
        fill_random(&mut raw)?;
        Ok(b64url_encode(&raw).into_bytes())
    }
}

/// The InvalidToken equivalents; every one of them is a `''` return to the
/// caller, never a plaintext guess.
#[derive(Debug)]
enum FernetError {
    Empty,
    WrongVersion,
    TooShort,
    BadCiphertextLength,
    SignatureMismatch,
    BadPkcs7Padding,
    ChainingFailed,
}

impl std::fmt::Display for FernetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            FernetError::Empty => "token must contain at least one byte",
            FernetError::WrongVersion => "token has wrong version",
            FernetError::TooShort => "Invalid Fernet token: too short",
            FernetError::BadCiphertextLength => "Invalid Fernet token: bad padding",
            FernetError::SignatureMismatch => "Signature check failed.",
            FernetError::BadPkcs7Padding => "Invalid token: bad padding",
            FernetError::ChainingFailed => "CBC operation failed",
        };
        f.write_str(text)
    }
}

fn fill_random(buf: &mut [u8]) -> Result<()> {
    // No clock-seeded fallback: a key that can be guessed from the wall clock is
    // worse than refusing to store the secret, and Python simply raises here.
    getrandom::fill(buf).map_err(|e| Error::Crypto(format!("secure random generation failed: {e}")))
}

fn cbc_encrypt(key: &[u8; SUBKEY_LEN], iv: &[u8; IV_LEN], plaintext: &[u8]) -> Vec<u8> {
    // PKCS7 always appends 1..=16 bytes, so the exact buffer size is known.
    let padded_len = plaintext.len() + (BLOCK_LEN - (plaintext.len() % BLOCK_LEN));
    let mut buf = vec![0u8; padded_len];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    let cipher = Aes128CbcEnc::new(GenericArray::from_slice(key), GenericArray::from_slice(iv));
    cipher
        .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
        .map(|ct| ct.to_vec())
        // `encrypt_padded_mut` cannot fail for a correctly sized buffer.
        .unwrap_or_default()
}

fn cbc_decrypt(
    key: &[u8; SUBKEY_LEN],
    iv: &[u8; IV_LEN],
    ciphertext: &mut [u8],
) -> std::result::Result<Vec<u8>, FernetError> {
    if ciphertext.is_empty() || ciphertext.len() % BLOCK_LEN != 0 {
        return Err(FernetError::BadCiphertextLength);
    }
    let cipher = Aes128CbcDec::new(GenericArray::from_slice(key), GenericArray::from_slice(iv));
    cipher
        .decrypt_padded_mut::<Pkcs7>(ciphertext)
        .map(|pt| pt.to_vec())
        .map_err(|_| FernetError::BadPkcs7Padding)
}

/// `Fernet.encrypt` minus the base64 wrapper.
fn fernet_encrypt_key(key: &FernetKey, plaintext: &[u8]) -> std::result::Result<Vec<u8>, Error> {
    let mut iv = [0u8; IV_LEN];
    fill_random(&mut iv)?;

    let timestamp = unix_timestamp();
    let mut token = Vec::with_capacity(TOKEN_HEADER_LEN + BLOCK_LEN + plaintext.len() + MAC_LEN);
    token.push(FERNET_VERSION);
    token.extend_from_slice(&timestamp.to_be_bytes());
    token.extend_from_slice(&iv);
    let ciphertext = cbc_encrypt(&key.encryption, &iv, plaintext);
    if ciphertext.is_empty() {
        return Err(Error::Crypto("AES-CBC encryption failed".to_string()));
    }
    token.extend_from_slice(&ciphertext);
    let mac = hmac_sha256(&key.signing, &token);
    token.extend_from_slice(&mac);
    Ok(token)
}

fn unix_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        // A clock before the epoch is not something Fernet can express; Python's
        // `int(time.time())` would have produced a negative value instead.
        .unwrap_or(0)
}

/// `Fernet.decrypt`: version, length, MAC-then-decrypt, in that order — the same
/// order `cryptography` uses, so a forged token is rejected before it is parsed.
/// No TTL is enforced because `crypto.py:89` calls `decrypt(token)` without one.
fn fernet_decrypt_key(key: &FernetKey, token: &[u8]) -> std::result::Result<Vec<u8>, FernetError> {
    if token.is_empty() {
        return Err(FernetError::Empty);
    }
    if token[0] != FERNET_VERSION {
        return Err(FernetError::WrongVersion);
    }
    if token.len() < TOKEN_MIN_LEN {
        return Err(FernetError::TooShort);
    }
    let body = &token[..token.len() - MAC_LEN];
    let expected = hmac_sha256(&key.signing, body);
    if !ct_eq(&expected, &token[token.len() - MAC_LEN..]) {
        return Err(FernetError::SignatureMismatch);
    }
    let mut ciphertext = body[TOKEN_HEADER_LEN..].to_vec();
    let iv: [u8; IV_LEN] = body[TOKEN_HEADER_LEN - IV_LEN..TOKEN_HEADER_LEN]
        .try_into()
        .map_err(|_| FernetError::ChainingFailed)?;
    cbc_decrypt(&key.encryption, &iv, &mut ciphertext)
}

// --------------------------------------------------------------- key storage

/// `crypto._default_key_path`: `os.path.join(DATA_DIR, 'encryption.key')`.
/// `paths::data_dir()` honours `READMD_DATA_DIR` exactly like `config.DATA_DIR`.
fn default_key_path() -> PathBuf {
    paths::data_dir().join("encryption.key")
}

fn default_vault_path() -> PathBuf {
    paths::data_dir().join("credentials.vault")
}

/// ASCII whitespace, which is what `bytes.strip()` removes in `crypto.py:46`.
fn strip_ascii_whitespace(bytes: &[u8]) -> &[u8] {
    let is_space = |b: &u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    let start = bytes.iter().position(|b| !is_space(b)).unwrap_or(0);
    let end = bytes
        .iter()
        .rposition(|b| !is_space(b))
        .map(|i| i + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

/// `crypto._get_or_create_key`. An existing file is authoritative: if it holds
/// something that is not a Fernet key we fail closed the way `Fernet(key)`
/// raising `ValueError` does, and never silently rotate the secret away.
fn get_or_create_key(key_path: Option<&Path>) -> Result<FernetKey> {
    let path = key_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(default_key_path);

    if path.exists() {
        let raw = fs::read(&path)?;
        return FernetKey::from_base64(strip_ascii_whitespace(&raw)).ok_or_else(|| {
            Error::Crypto(format!(
                "encryption key file is not a Fernet key: {}",
                path.display()
            ))
        });
    }

    let key_bytes = FernetKey::generate_base64()?;
    if let Some(parent) = path.parent() {
        // `os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)`.
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(&path, &key_bytes).map_err(|e| {
        Error::Crypto(format!(
            "Cannot write encryption key {}: {}",
            path.display(),
            e
        ))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // `if os.name != 'nt': os.chmod(path, 0o600)` inside `try/OSError`.
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }

    FernetKey::from_base64(&key_bytes)
        .ok_or_else(|| Error::Crypto("generated key rejected".to_string()))
}

// ------------------------------------------------------------- public surface

/// Python keeps this flag because `cryptography` may be missing at runtime
/// (`crypto.py:23-27`). The kernel links its own Fernet implementation, so the
/// write path can never degrade to plaintext storage.
pub fn is_crypto_available() -> bool {
    true
}

/// `crypto.encrypt_api_key`: `'enc:' + Fernet(key).encrypt(value)`.
///
/// Every failure is an `Err` — Python logs and raises `RuntimeError`, because
/// storing an API key unencrypted is not an acceptable fallback.
pub fn encrypt_api_key(api_key: &str, key_path: Option<&Path>) -> Result<String> {
    if api_key.is_empty() {
        return Ok(String::new());
    }
    let result = (|| -> Result<String> {
        let key = get_or_create_key(key_path)?;
        let token = fernet_encrypt_key(&key, api_key.as_bytes())?;
        Ok(format!("enc:{}", b64url_encode(&token)))
    })();
    if let Err(e) = &result {
        // `logging.error("API Key 加密失败，拒绝保存: %s", e)`
        log::error!("API Key 加密失败，拒绝保存: {e}");
    }
    result
}

/// `crypto.decrypt_api_key`. Never fails: an unusable token yields `''`, which
/// is how the legacy app signals "no key configured" to the AI layer.
pub fn decrypt_api_key(encrypted_key: &str, key_path: Option<&Path>) -> Result<String> {
    if encrypted_key.is_empty() {
        return Ok(String::new());
    }
    // `crypto.py:79-81`: anything without the marker is a plaintext key on disk
    // and is refused rather than echoed back.
    let cipher_text = match encrypted_key.strip_prefix("enc:") {
        Some(rest) => rest,
        None => {
            log::error!("检测到未加密的 API Key，拒绝读取");
            return Ok(String::new());
        }
    };

    let fail = |reason: String| {
        // `except InvalidToken` logs the mismatch wording, everything else the
        // generic one; both return `''`, so the distinction is log-only.
        log::warn!("API Key 解密失败: {reason}");
        Ok(String::new())
    };

    let key = match get_or_create_key(key_path) {
        Ok(key) => key,
        Err(e) => {
            log::warn!("已加密的 API Key 无法解密，缺少可用密钥: {e}");
            return Ok(String::new());
        }
    };

    let token = match b64url_decode_lenient(cipher_text) {
        Some(token) => token,
        None => {
            log::warn!("API Key 解密密钥不匹配或密文损坏");
            return Ok(String::new());
        }
    };

    match fernet_decrypt_key(&key, &token) {
        Ok(plaintext) => match String::from_utf8(plaintext) {
            Ok(text) => Ok(text),
            // `.decode('utf-8')` raising is the generic `except Exception` arm.
            Err(e) => fail(format!("plaintext is not valid utf-8: {e}")),
        },
        Err(e) => {
            log::warn!("API Key 解密密钥不匹配或密文损坏: {e}");
            Ok(String::new())
        }
    }
}

/// `crypto._credential_target`: `strip()` then `re.fullmatch` of
/// `^cred:[A-Za-z0-9_-]{8,128}$` (`_CREDENTIAL_RE`, `crypto.py:21`), and the
/// vault/keychain name is `SERVICE_NAME + '/' + cid` with the `cred:` prefix
/// kept inside the id.  `regex` is not used here, so the class is scanned by
/// hand and stays ASCII-only the way CPython's character class is.
fn validate_credential_id(credential_id: &str) -> Result<String> {
    let cid = credential_id.trim();
    let body = match cid.strip_prefix("cred:") {
        Some(b) => b,
        None => return Err(Error::Crypto("invalid credential id".to_string())),
    };
    let len = body.chars().count();
    let valid = (8..=128).contains(&len)
        && body
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !valid {
        return Err(Error::Crypto("invalid credential id".to_string()));
    }

    Ok(format!("{}/{}", SERVICE_NAME, cid))
}

fn vault_path() -> PathBuf {
    default_vault_path()
}

/// `crypto._vault_write`: one `enc:` line plus `\n` into `credentials.vault.tmp`,
/// then `os.replace` onto the real file.
fn vault_write(value: &serde_json::Value) -> Result<()> {
    let json_str = serde_json::to_string(value)
        .map_err(|e| Error::Crypto(format!("JSON serialization failed: {}", e)))?;

    let encrypted = encrypt_api_key(&json_str, None)?;
    let path = vault_path();
    // `os.makedirs(DATA_DIR, exist_ok=True)` at `crypto.py:113`.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // `_vault_path() + '.tmp'` — the suffix is appended to the full file name.
    let mut tmp_os = path.as_os_str().to_os_string();
    tmp_os.push(".tmp");
    let tmp_path = PathBuf::from(tmp_os);

    // `handle.write(encrypted + '\n')` with `newline='\n'`.
    let mut payload = encrypted.into_bytes();
    payload.push(b'\n');
    fs::write(&tmp_path, payload)
        .map_err(|e| Error::Crypto(format!("Write temp vault failed: {}", e)))?;

    fs::rename(&tmp_path, &path).map_err(|e| Error::Crypto(format!("Rename vault failed: {}", e)))
}

/// `crypto._vault_load`: unreadable, undecryptable, non-dict and non-JSON all
/// collapse to an empty vault instead of an error.
fn vault_load() -> Result<serde_json::Value> {
    let vault_path = vault_path();

    if !vault_path.is_file() {
        return Ok(serde_json::Value::Object(Map::new()));
    }

    let content = fs::read_to_string(&vault_path)
        .map_err(|e| Error::Crypto(format!("Read vault failed: {}", e)))?;

    // `_vault_load`: `decrypt_api_key(handle.read().strip())`.
    let decrypted = decrypt_api_key(content.trim(), None)?;

    if decrypted.is_empty() {
        return Ok(serde_json::Value::Object(Map::new()));
    }

    // `data if isinstance(data, dict) else {}`, `json.JSONDecodeError` included.
    let value: serde_json::Value = match serde_json::from_str(&decrypted) {
        Ok(v @ serde_json::Value::Object(_)) => v,
        _ => serde_json::Value::Object(Map::new()),
    };

    Ok(value)
}

// ------------------------------------------------------- native credential tier
//
// `crypto._native_store` / `_native_load` / `_native_delete` (`crypto.py:136-205`)
// are the **first** tier of Python's storage ladder: `store_credential` returns
// `'native'` as soon as `_native_store` succeeds and only ever reaches
// `credentials.vault` through its fallback (`crypto.py:212-217`), and
// `load_credential` consults the OS store before the vault (`crypto.py:224-228`).
//
// The Windows branch of that ladder is `win32cred` (`crypto.py:141-148`,
// `crypto.py:171-176`, `crypto.py:194-196`), i.e. the `advapi32` Credential
// Manager entry points.  The kernel calls the same three functions directly
// through `extern "system"`, so the tier is real without adding a crate and
// without spawning anything — the whole point of the native-independence rule.
// The record is field-for-field what `crypto.py` builds:
//
// ```text
// Type           = CRED_TYPE_GENERIC          (win32cred value 1)
// TargetName     = SERVICE_NAME + '/' + cid   (`_credential_target`, same key)
// UserName       = SERVICE_NAME               ("ReadMD")
// CredentialBlob = secret.encode('utf-8')     (raw UTF-8, *not* UTF-16)
// Persist        = CRED_PERSIST_LOCAL_MACHINE (win32cred value 2)
// Comment        = "ReadMD provider credential"
// ```
//
// The blob stays UTF-8 because that is what the authority writes *and* what
// `_native_load` reads back with `blob.decode('utf-8')` (`crypto.py:174`); a
// Python build whose `CredWrite` works therefore sees exactly what the kernel
// stored, and vice versa.
//
// macOS (`security`) and Linux (`secret-tool`) reach their keychains through
// `subprocess.run` (`crypto.py:152-163`, `crypto.py:177-186`).  Those are
// external programs, so the kernel does **not** follow them there: the tier
// reports "unavailable" and Python's own fallback branch — the encrypted vault —
// carries the value.  Nothing is ever weakened to plaintext: `crypto.py` has no
// third tier at all (a missing crypto backend is a hard error at
// `crypto.py:64` and `crypto.py:112`), and `_vault_write` still fails closed.

/// Test-only mirror of what
/// `scratch/rust_parity/fernet_python_store_credential.py:30-32` does to the
/// Python module (`crypto._native_store = lambda *a: False`): a box with a
/// working keychain must still be able to produce/read *vault* fixtures, which
/// is what the cross-language vault vectors are.  Never set outside `cargo test`.
#[cfg(test)]
static NATIVE_TIER_DISABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(not(test))]
#[inline]
fn native_tier_disabled() -> bool {
    false
}

#[cfg(test)]
#[inline]
fn native_tier_disabled() -> bool {
    NATIVE_TIER_DISABLED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Disable/enable the OS tier for the duration of one vault-tier test.
#[cfg(test)]
fn with_native_tier_disabled<T>(run: impl FnOnce() -> T) -> T {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            NATIVE_TIER_DISABLED.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    NATIVE_TIER_DISABLED.store(true, std::sync::atomic::Ordering::SeqCst);
    let _guard = Guard;
    run()
}

#[cfg(windows)]
mod win_credentials {
    //! `advapi32` Credential Manager, declared by hand the way `win32cred`
    //! wraps it.  Same precedent as the `kernel32` / `shell32` blocks in
    //! `main.rs` and `ai.rs`; no dependency, no child process.
    use core::ffi::c_void;

    /// `win32cred.CRED_TYPE_GENERIC`
    const CRED_TYPE_GENERIC: u32 = 1;
    /// `win32cred.CRED_PERSIST_LOCAL_MACHINE`
    const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
    /// The `Flags` argument `crypto.py:148` passes as `0`; `CredRead` /
    /// `CredDelete` take their flags the same way (pywin32 defaults them to 0).
    const NO_FLAGS: u32 = 0;

    /// `FILETIME` (`wincred.h:478-514` declares `LastWritten` as a `FILETIME`,
    /// i.e. two `DWORD`s = 8 bytes, **not** a 16-byte `SYSTEMTIME`; using
    /// `SYSTEMTIME` here misaligns the record to 88 bytes and every `CredWriteW`
    /// answers Win32 error 87 `ERROR_INVALID_PARAMETER`).  The OS fills it in for
    /// us, so the value we pass is ignored.
    #[repr(C)]
    #[derive(Default)]
    struct Filetime {
        dw_low_date_time: u32,
        dw_high_date_time: u32,
    }

    /// `CREDENTIALW` (`wincred.h:478-514`) in declaration order.  `repr(C)` gets
    /// the alignment holes for free (`CredentialBlobSize` is a `DWORD` followed
    /// by an 8-byte-aligned pointer), so no manual filler is needed; on
    /// x86_64 this measures 80 bytes, which is what the standalone FFI probe in
    /// `scratch/rust_parity/_crypto_native_ffi_probe.rs` confirmed round-trips.
    #[repr(C)]
    struct CredentialW {
        flags: u32,
        credential_type: u32,
        target_name: *mut u16,
        comment: *mut u16,
        last_written: Filetime,
        credential_blob_size: u32,
        credential_blob: *mut u8,
        persist: u32,
        attribute_count: u32,
        attributes: *mut c_void,
        target_alias: *mut u16,
        user_name: *mut u16,
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn CredWriteW(credential: *const CredentialW, flags: u32) -> i32;
        fn CredReadW(
            target_name: *const u16,
            credential_type: u32,
            flags: u32,
            credential: *mut *mut CredentialW,
        ) -> i32;
        fn CredDeleteW(target_name: *const u16, credential_type: u32, flags: u32) -> i32;
        fn CredFree(credential: *mut c_void);
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLastError() -> u32;
    }

    fn to_wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(core::iter::once(0)).collect()
    }

    /// `win32cred.CredWrite({...}, 0)`.  `Err(code)` is the exception Python
    /// swallows at `crypto.py:150-151` before falling through to the vault.
    pub(super) fn write(target: &str, user: &str, comment: &str, blob: &[u8]) -> Result<(), u32> {
        let target_w = to_wide(target);
        let user_w = to_wide(user);
        let comment_w = to_wide(comment);
        let record = CredentialW {
            flags: 0,
            credential_type: CRED_TYPE_GENERIC,
            target_name: target_w.as_ptr() as *mut u16,
            comment: comment_w.as_ptr() as *mut u16,
            last_written: Filetime::default(),
            credential_blob_size: blob.len() as u32,
            credential_blob: blob.as_ptr() as *mut u8,
            persist: CRED_PERSIST_LOCAL_MACHINE,
            attribute_count: 0,
            attributes: core::ptr::null_mut(),
            target_alias: core::ptr::null_mut(),
            user_name: user_w.as_ptr() as *mut u16,
        };
        // SAFETY: every pointer member points into a live `Vec`/slice for the
        // whole call, and `record` is only read by the OS.
        let ok = unsafe { CredWriteW(&record, NO_FLAGS) };
        if ok != 0 {
            return Ok(());
        }
        // SAFETY: only valid immediately after the failed call above.
        Err(unsafe { GetLastError() })
    }

    /// `win32cred.CredRead(target, CRED_TYPE_GENERIC)`.  `None` is "no such
    /// record or the call raised", both of which Python collapses to the vault
    /// lookup via `except Exception` (`crypto.py:175-176`).
    pub(super) fn read(target: &str) -> Option<Vec<u8>> {
        let target_w = to_wide(target);
        let mut record: *mut CredentialW = core::ptr::null_mut();
        // SAFETY: `target_w` outlives the call and `&mut record` is a valid
        // out-parameter.
        let ok = unsafe { CredReadW(target_w.as_ptr(), CRED_TYPE_GENERIC, NO_FLAGS, &mut record) };
        if ok == 0 || record.is_null() {
            return None;
        }
        // SAFETY: the OS just handed out a readable `CREDENTIALW` whose blob
        // spans `credential_blob_size` bytes.
        let blob = unsafe {
            let cred = &*record;
            if cred.credential_blob.is_null() {
                Vec::new()
            } else {
                core::slice::from_raw_parts(cred.credential_blob, cred.credential_blob_size as usize)
                    .to_vec()
            }
        };
        // SAFETY: `record` is the buffer `CredReadW` allocated; it is copied out
        // above and not used again.
        unsafe { CredFree(record as *mut c_void) };
        Some(blob)
    }

    /// `win32cred.CredDelete(target, CRED_TYPE_GENERIC)`.
    pub(super) fn delete(target: &str) -> Result<(), u32> {
        let target_w = to_wide(target);
        // SAFETY: `target_w` outlives the call.
        let ok = unsafe { CredDeleteW(target_w.as_ptr(), CRED_TYPE_GENERIC, NO_FLAGS) };
        if ok != 0 {
            return Ok(());
        }
        Err(unsafe { GetLastError() })
    }
}

/// Windows body of `crypto._native_store` (`crypto.py:139-152`).
#[cfg(windows)]
fn native_store_platform(target: &str, secret: &str) -> bool {
    match win_credentials::write(
        target,
        SERVICE_NAME,
        "ReadMD provider credential",
        secret.as_bytes(),
    ) {
        Ok(()) => true,
        // `logging.debug('Windows Credential Manager unavailable', exc_info=True)`
        // is the whole recovery: the exception is swallowed and the ladder
        // continues into the encrypted vault.
        Err(code) => {
            log::debug!("Windows Credential Manager unavailable: Win32 error {code}");
            false
        }
    }
}

/// Non-Windows body of `crypto._native_store`: `crypto.py:152-163` reaches the
/// macOS keychain and libsecret through the `security` / `secret-tool`
/// programs.  The kernel never spawns a program, so the tier reports itself
/// unavailable and `store_credential` takes Python's own fallback branch.
#[cfg(not(windows))]
#[allow(unused_variables)]
fn native_store_platform(target: &str, secret: &str) -> bool {
    false
}

/// Windows body of `crypto._native_load` (`crypto.py:170-177`).
#[cfg(windows)]
fn native_load_platform(target: &str) -> String {
    match win_credentials::read(target) {
        // `blob.decode('utf-8')` raising is inside the `except Exception` arm,
        // so an undecodable blob is indistinguishable from no record.
        Some(blob) => String::from_utf8(blob).unwrap_or_default(),
        None => String::new(),
    }
}

/// Non-Windows body of `crypto._native_load`, same reasoning as above.
#[cfg(not(windows))]
#[allow(unused_variables)]
fn native_load_platform(target: &str) -> String {
    String::new()
}

/// Windows body of `crypto._native_delete` (`crypto.py:193-198`).
#[cfg(windows)]
fn native_delete_platform(target: &str) -> bool {
    match win_credentials::delete(target) {
        Ok(()) => true,
        // `except Exception: pass`.
        Err(code) => {
            log::debug!("Windows Credential Manager delete failed: Win32 error {code}");
            false
        }
    }
}

/// Non-Windows body of `crypto._native_delete`.  Python only deletes through
/// `security` on macOS and not at all on Linux (`crypto.py:199-204`), so the
/// vault cleanup in `delete_credential` is the whole story here.
#[cfg(not(windows))]
#[allow(unused_variables)]
fn native_delete_platform(target: &str) -> bool {
    false
}

/// `crypto._native_store`.  `Ok(false)` is Python's `return False` — tier
/// unavailable or write failed, so the caller must use the vault.  The only
/// `Err` is the `ValueError` that `_credential_target` raises at
/// `crypto.py:137`, which Python lets escape `store_credential`.
fn native_store(credential_id: &str, secret: &str) -> Result<bool> {
    let target = validate_credential_id(credential_id)?;
    if native_tier_disabled() {
        return Ok(false);
    }
    Ok(native_store_platform(&target, secret))
}

/// `crypto._native_load`.  A missing record, a failed `CredRead`, a blob that is
/// not valid UTF-8 and a zero-length blob all yield `""`, which is exactly how
/// `crypto.py:175-187` reaches `return ''` and lets `load_credential` fall back
/// to the vault (`crypto.py:225-226`).
fn native_load(credential_id: &str) -> Result<String> {
    let target = validate_credential_id(credential_id)?;
    if native_tier_disabled() {
        return Ok(String::new());
    }
    Ok(native_load_platform(&target))
}

/// `crypto._native_delete`.  `delete_credential` throws the result away
/// (`crypto.py:235`) and cleans the vault regardless, so this is best-effort.
fn native_delete(credential_id: &str) -> Result<bool> {
    let target = validate_credential_id(credential_id)?;
    if native_tier_disabled() {
        return Ok(false);
    }
    Ok(native_delete_platform(&target))
}

#[derive(Debug, Clone, PartialEq)]
pub enum StoreBackend {
    Native,
    EncryptedVault,
}

/// `crypto.store_credential` (`crypto.py:208-217`): the OS credential store
/// first — `'native'` — and the encrypted vault only as its fallback.
///
/// There is no plaintext tier to fall back to and none is added: a secret the
/// native tier accepted is never duplicated into the vault (Python writes the
/// vault only when `_native_store` returns false), and a secret the native tier
/// refused is still persisted encrypted, never dropped and never in the clear.
///
/// Two consequences for the "could a value be silently swallowed" question, both
/// inherited from the authority: a *stale* vault entry for the same target
/// survives a successful native store (`crypto.py:212-213` returns before the
/// vault is opened), and that is harmless because `load_credential` checks the OS
/// store first (`crypto.py:224-226`), so the stronger tier always wins; and a
/// native write that fails leaves `store_credential` to run the vault branch, so
/// the value is always persisted somewhere.
pub fn store_credential(credential_id: &str, secret: &str) -> Result<StoreBackend> {
    if secret.is_empty() {
        return Err(Error::Crypto("credential secret is empty".to_string()));
    }

    // `crypto.py:212-213`: the native tier wins outright.
    if native_store(credential_id, secret)? {
        return Ok(StoreBackend::Native);
    }

    let target = validate_credential_id(credential_id)?;
    let mut vault = vault_load()?.as_object().cloned().unwrap_or_default();
    let encrypted = encrypt_api_key(secret, None)?;
    vault.insert(target, serde_json::Value::String(encrypted));

    vault_write(&serde_json::Value::Object(vault))?;

    Ok(StoreBackend::EncryptedVault)
}

/// `crypto.load_credential` (`crypto.py:220-228`): native first, non-empty
/// native wins, otherwise the encrypted vault under the same target name.
pub fn load_credential(credential_id: &str) -> Result<String> {
    if credential_id.is_empty() {
        return Ok(String::new());
    }

    let native = native_load(credential_id)?;
    if !native.is_empty() {
        return Ok(native);
    }

    let target = validate_credential_id(credential_id)?;
    let vault = vault_load()?;

    if let Some(serde_json::Value::String(encrypted)) = vault.get(&target) {
        return decrypt_api_key(encrypted, None);
    }

    Ok(String::new())
}

/// `crypto.delete_credential` (`crypto.py:231-239`): best-effort native delete
/// plus the vault cleanup, which always runs.
pub fn delete_credential(credential_id: &str) -> Result<()> {
    if credential_id.is_empty() {
        return Ok(());
    }

    // `crypto.py:235` ignores the return value; the vault branch below is not
    // conditional on it.
    native_delete(credential_id)?;

    let target = validate_credential_id(credential_id)?;
    let mut vault = vault_load()?.as_object().cloned().unwrap_or_default();

    // `if _credential_target(cid) in vault: vault.pop(...); _vault_write(vault)`
    // — an empty dict is still written back, the file is never unlinked.
    if vault.remove(&target).is_some() {
        vault_write(&serde_json::Value::Object(vault))?;
    }

    Ok(())
}

// --------------------------------------------------------- digest primitives

/// `hashlib.sha256(data).hexdigest().lower()`
pub fn sha256_hex(bytes: &[u8]) -> String {
    let out = sha256(bytes);
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// `updater.compute_file_sha256`: the same 65536-byte chunking, so a digest
/// computed here is byte-for-byte what the Python updater would have written
/// into `SHA256SUMS.txt` for the same file.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = read_chunk(&mut f, &mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    Ok(s)
}

/// `f.read(65536)` semantics: a short read is fine, only a clean EOF stops the
/// loop, and `Interrupted` retries like Python's EINTR-handling read would.
fn read_chunk(f: &mut fs::File, buf: &mut [u8]) -> io::Result<usize> {
    use std::io::Read;
    loop {
        match f.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Written by `scratch/rust_parity/fernet_emit_vectors.py` using the
    /// installed `cryptography` 50.0.0, then pasted by
    /// `scratch/rust_parity/fernet_emit_rust_block.py` — no hand transcription.
    /// `(name, key, plaintext, token-without-enc-prefix)`.
    const VECTORS: &[(&str, &str, &str, &str)] = &[
        ("ascii_short", "pPe_wNh6hrPftQ6sjcdKIg4pll6LqGax5Dn9to3cmNA=", "sk-test-1234567890", "gAAAAABqsisndJ9xBPGDHLct3HAqM7DhjR_HOTrcWe0TYRXYjvSvZ4mr8HsqVdF5B01teMTGWlatHEE372fsHMjTJVnT8anMajHBtv8Yk9dIUTr-qoKehFs="),
        ("block_aligned_64", "Vi4PsRGpIJwOlb5KL5qL7YksXE1kwoIln2as9RSWzvs=", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "gAAAAABqsisnSLto7zg3i5MMpkkWoJ3cfqN492HUyDSTH19KmxcX5Jisqdr_ahwTafcU6JGOl0iP2KTrTYgTTHeKAJ_Mxmdwxlj8B5-9YPuu7Sch640V_Qn8hSKjWMmIHx9Vph4mxDIRx7vxj2P7b-MDlqgIDMsCTmB9TrYjAL6WSsKHqrZeLTA="),
        ("cjk_utf8", "HwtW1cKjd5zZo6SER6OefVqD5_8ig3o9x2H4PhnZkaY=", "sk-\u{6d4b}\u{8bd5}\u{5bc6}\u{94a5}-\u{1f511}-\u{4e2d}\u{6587}\u{4e0e}emoji", "gAAAAABqsisnKjvVoyb7tHQiLhH6rlXzAh9yLifNruk8eZFWgxSd74JkHCY0BPYb2Q7DudgVtfH8iM1kuttOLd8M8q_J3a-GrZxzyDedd8mMbi_x3vW06faPAWprP8BCEDjlZ3wxuitn"),
        ("vault_envelope", "ji-NxpWLkzsCVpa4NUmV9uHpg5RlKpS_nw4z2LTOxIw=", "{\"ReadMD/cred:abcd1234\":\"enc:gAAAAABqsisnCeVHNh6ZuhBYOEPgBpOTIzTq3fe_i8uuYFtrf_27brS12lWcRuM9gO3dBlYTy0Qp1PlaSKt8YGJ3XXhrJknl5ep62qIXRRp2lP0fQ-spTO8=\"}", "gAAAAABqsisnMglXfc_-UcRAVxwrWjG5T3imQuwrJ7_MG7PLyyzYxeFkzhjCXjxdEWaLWm14DendRHp1N_1SPjc5BlCDxk_FXTw90H0EjwOt690e-EI2We1kdfaIPA9BWMo-ra2ficUd_UH733emYhjtvbPik5XdyEpVVM4DolEA9rFqNle73VPfGrDlSsBRC1fmkIMFGtNvLUxGCcwaR3dBfeZ64eVmegreVTUNmqiegVTLDdcUFqR0ovu311yOYr2lnbj2ifu3pS4wuPqYmJ-erialdDZYmg=="),
        ("stale_timestamp", "gZOVNa1bz-2edwV-VDGUgIQW3Yx9WCnWvVRqcU6KF3A=", "sk-from-two-thousand-and-five", "gAAAAABX5ignRy-Fh5c8y_9__nZ696rRZaPs_rRN5VzatCJsY1L9dLXt6y8E8MufLOcu48VMvcXLvXF-TyRAGZajZXuskwVmfponLUfiK-X1pIk11kY38MY="),
    ];

    /// Serialises the tests that repoint the process-wide data directory.
    fn data_dir_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn key_file(dir: &Path, name: &str, key: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, key.as_bytes()).unwrap();
        path
    }

    /// The hard parity claim: five keys and five tokens produced by Python's
    /// `cryptography` open in Rust with nothing but the key file shared.
    #[test]
    fn decrypts_cryptography_python_vectors() {
        let tmp = tempfile::tempdir().unwrap();
        for (name, key, plaintext, token) in VECTORS {
            let path = key_file(tmp.path(), &format!("{name}.key"), key);
            let got = decrypt_api_key(&format!("enc:{token}"), Some(&path))
                .unwrap_or_else(|e| panic!("{name}: decrypt errored: {e}"));
            assert_eq!(&got, plaintext, "{name}: Rust did not read Python's token");
        }
    }

    /// And the same vectors without the `enc:` marker must fail closed, exactly
    /// like `crypto.py:79-81` refuses a legacy plaintext value.
    #[test]
    fn refuses_values_without_the_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let (_name, key, _, token) = VECTORS[0];
        let path = key_file(tmp.path(), "plain.key", key);
        assert_eq!(decrypt_api_key(token, Some(&path)).unwrap(), "");
        assert_eq!(
            decrypt_api_key("sk-plaintext-key", Some(&path)).unwrap(),
            ""
        );
        assert_eq!(decrypt_api_key("", Some(&path)).unwrap(), "");
        assert_eq!(encrypt_api_key("", Some(&path)).unwrap(), "");
    }

    /// A key file may carry the trailing newline a shell wrote; Python `.strip()`s
    /// it (`crypto.py:46`) so the vectors must still open, and a token that lost
    /// its padding must not (`InvalidToken`).
    #[test]
    fn matches_python_token_framing() {
        let tmp = tempfile::tempdir().unwrap();
        let (_name, key, plaintext, token) = VECTORS[1];
        let path = key_file(tmp.path(), "ws.key", &format!("{key}\n"));
        assert_eq!(
            decrypt_api_key(&format!("enc:{token}"), Some(&path)).unwrap(),
            plaintext
        );

        let unpadded = token.trim_end_matches('=');
        let path = key_file(tmp.path(), "ws2.key", key);
        assert_eq!(
            decrypt_api_key(&format!("enc:{unpadded}"), Some(&path)).unwrap(),
            ""
        );
    }

    /// Wrong key, tampered bytes, truncated token, bad version: every one of them
    /// is Python's `InvalidToken` and must yield `''`, never a guess.
    #[test]
    fn invalid_token_equivalents_fail_closed() {
        let tmp = tempfile::tempdir().unwrap();
        let (_name, key, _, token) = VECTORS[2];
        let path = key_file(tmp.path(), "tamper.key", key);
        let other = key_file(tmp.path(), "other.key", VECTORS[3].1);
        let raw = b64url_decode_lenient(token).unwrap();

        let flip = |byte: usize| {
            let mut tampered = raw.clone();
            tampered[byte] ^= 0x01;
            format!("enc:{}", b64url_encode(&tampered))
        };

        assert_eq!(
            decrypt_api_key(&format!("enc:{token}"), Some(&other)).unwrap(),
            ""
        );
        assert_eq!(decrypt_api_key(&flip(0), Some(&path)).unwrap(), ""); // version
        assert_eq!(decrypt_api_key(&flip(10), Some(&path)).unwrap(), ""); // IV
        assert_eq!(decrypt_api_key(&flip(40), Some(&path)).unwrap(), ""); // ciphertext
        assert_eq!(
            decrypt_api_key(&flip(raw.len() - 1), Some(&path)).unwrap(),
            "",
            "MAC byte flipped"
        );
        // Truncate to just over the MAC, and to below the minimum length.
        let short = format!("enc:{}", b64url_encode(&raw[..TOKEN_MIN_LEN - 16]));
        assert_eq!(decrypt_api_key(&short, Some(&path)).unwrap(), "");
        assert_eq!(
            decrypt_api_key("enc:!!!not base64!!!", Some(&path)).unwrap(),
            ""
        );
        assert_eq!(decrypt_api_key("enc:", Some(&path)).unwrap(), "");
    }

    /// Re-encrypting each vector's plaintext in Rust yields a fresh, valid token
    /// that Python-side consumers accept, and that Rust itself can read back.
    #[test]
    fn re_encrypts_python_vectors_to_valid_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        for (name, key, plaintext, _) in VECTORS {
            let path = key_file(tmp.path(), &format!("re_{name}.key"), key);
            let token = encrypt_api_key(plaintext, Some(&path)).unwrap();
            assert!(token.starts_with("enc:"), "{name}: marker missing");

            let raw = b64url_decode_lenient(&token[4..]).expect("self token not b64url");
            assert_eq!(raw[0], FERNET_VERSION, "{name}: version byte");
            assert!(raw.len() >= TOKEN_MIN_LEN, "{name}: token too short");
            assert_eq!(
                (raw.len() - TOKEN_MIN_LEN) % BLOCK_LEN,
                0,
                "{name}: ciphertext is not block aligned"
            );
            let body = &raw[..raw.len() - MAC_LEN];
            let parsed = FernetKey::from_base64(key.as_bytes()).expect("python key");
            let mac = hmac_sha256(&parsed.signing, body);
            assert!(
                ct_eq(&mac, &raw[raw.len() - MAC_LEN..]),
                "{name}: HMAC does not verify"
            );
            // The 8-byte big-endian timestamp must be "now", like int(time.time()).
            let ts = u64::from_be_bytes(body[1..9].try_into().unwrap());
            let now = unix_timestamp();
            assert!(
                ts <= now && now - ts < 120,
                "{name}: timestamp {ts} vs {now}"
            );

            assert_eq!(&decrypt_api_key(&token, Some(&path)).unwrap(), plaintext);
        }
    }

    /// The key file is on-disk-compatible: 44 base64url characters, no newline,
    /// generated once and reused, and never rewritten when it is unreadable.
    #[test]
    fn generates_and_reuses_python_readable_key_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("encryption.key");

        let token = encrypt_api_key("sk-generated-by-rust", Some(&path)).unwrap();
        let raw = fs::read(&path).unwrap();
        assert_eq!(raw.len(), 44, "key file must be one 44-char Fernet key");
        assert!(
            !raw.contains(&b'\n'),
            "Python writes the key without a newline"
        );
        assert!(
            std::str::from_utf8(&raw)
                .unwrap()
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '='),
            "url-safe alphabet only"
        );
        assert!(
            path.parent().unwrap().is_dir(),
            "parent directory is created"
        );

        // Reuse: a second call must not rotate the key away.
        assert_eq!(fs::read(&path).unwrap(), raw);
        assert_eq!(
            decrypt_api_key(&token, Some(&path)).unwrap(),
            "sk-generated-by-rust"
        );

        // Fail closed on a corrupt key file instead of silently regenerating it.
        let bad = tmp.path().join("bad.key");
        fs::write(&bad, b"not-a-fernet-key-at-all-because-too-short").unwrap();
        assert!(encrypt_api_key("sk-x", Some(&bad)).is_err());
        assert_eq!(decrypt_api_key(&token, Some(&bad)).unwrap(), "");
        assert_eq!(
            fs::read(&bad).unwrap(),
            b"not-a-fernet-key-at-all-because-too-short".to_vec(),
            "an existing key file is never overwritten"
        );
    }

    /// HMAC-SHA256 checked against RFC 4231 test cases 1-3 (the hand-rolled
    /// construction, not a library we no longer depend on).
    #[test]
    fn hmac_sha256_matches_rfc4231() {
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(
            hex(&hmac_sha256(&[0x0b; 20], b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        assert_eq!(
            hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        assert_eq!(
            hex(&hmac_sha256(&[0xaa; 20], &[0xdd; 50])),
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
        );
        // Keys longer than the SHA-256 block are hashed first, and an empty key
        // is still legal.  Both tags come from `hmac.new(k, m, sha256)`.
        assert_eq!(
            hex(&hmac_sha256(
                &[0x4a; 80],
                b"Test using a larger key size than the block size"
            )),
            "ad3d4ad67ad18829c673f8729ed33a963f1c71440a1aa5b9d0ba107c3083a3ce"
        );
        assert_eq!(
            hex(&hmac_sha256(&[], b"empty")),
            "fb5c2dafa48d4480ffef8816169cb015891e168c0fe14a5ec8edc410dd954500"
        );
    }

    /// A token the old AES-256-GCM writer would have produced must not open any
    /// more: the point of deleting that path rather than keeping it as a
    /// fallback is that garbage must not be mistaken for a secret.
    #[test]
    fn legacy_rust_gcm_payload_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = key_file(tmp.path(), "gcm.key", VECTORS[0].1);
        // 12-byte nonce + 16-byte GCM tag + payload, standard base64 (`enc:` +
        // base64::STANDARD), the format removed in this change.
        let mut payload = vec![0u8; 12 + 16 + 8];
        for (i, b) in payload.iter_mut().enumerate() {
            *b = (i * 7) as u8;
        }
        let legacy = format!(
            "enc:{}",
            base64::engine::general_purpose::STANDARD.encode(&payload)
        );
        assert_eq!(decrypt_api_key(&legacy, Some(&path)).unwrap(), "");
    }

    /// Cross-language hand-off artefact: `fernet_python_reads_rust.py` decrypts
    /// this file with `cryptography` and asserts every `expected` matches.
    #[test]
    fn exports_rust_tokens_for_python_to_verify() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cases = Vec::new();
        for (name, key, plaintext, _) in VECTORS {
            let path = key_file(tmp.path(), &format!("export_{name}.key"), key);
            let token = encrypt_api_key(plaintext, Some(&path)).unwrap();
            cases.push(serde_json::json!({
                "name": name,
                "key": key,
                "expected": plaintext,
                "prefixed": token,
            }));
        }
        let out = serde_json::json!({ "cases": cases });
        let dest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../scratch/rust_parity/rust_tokens.json");
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&dest, serde_json::to_vec_pretty(&out).unwrap())
            .expect("write rust_tokens.json for the Python side");
        assert!(dest.is_file());
        assert_eq!(
            out["cases"].as_array().map(|v| v.len()),
            Some(VECTORS.len())
        );
    }

    /// `scratch/rust_parity/fernet_live_vault_probe.py` copies the real
    /// `%APPDATA%\ReadMD\encryption.key` + `credentials.vault` that the shipped
    /// Python app wrote, plus the SHA-256 of every secret it could decrypt.  If
    /// Rust reads the same two files to the same digests, the on-disk formats
    /// are interchangeable for actual user data, not just for fixtures.
    #[test]
    fn reads_the_real_python_written_vault() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scratch/rust_parity");
        let expected_path = dir.join("live_expected.json");
        let key_path = dir.join("live_vault").join("encryption.key");
        let vault_path = dir.join("live_vault").join("credentials.vault");
        if !(expected_path.is_file() && key_path.is_file() && vault_path.is_file()) {
            eprintln!("live fixtures absent; run fernet_live_vault_probe.py first");
            return;
        }

        let expected: serde_json::Value =
            serde_json::from_slice(&fs::read(&expected_path).unwrap()).unwrap();
        let entries = expected["entries"].as_object().unwrap();
        assert!(
            !entries.is_empty(),
            "the probe recorded no vault entries to compare"
        );

        // `crypto._vault_load`: `decrypt_api_key(handle.read().strip())`.
        let raw = fs::read_to_string(&vault_path).unwrap();
        let decrypted = decrypt_api_key(raw.trim(), Some(&key_path)).unwrap();
        assert!(
            !decrypted.is_empty(),
            "Rust could not open the vault Python wrote"
        );
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&decrypted).unwrap();
        assert_eq!(map.len(), entries.len(), "vault entry count differs");

        for (target, want) in entries {
            let entry = map
                .get(target)
                .unwrap_or_else(|| panic!("{target} missing"));
            let encrypted = entry.as_str().unwrap();
            assert!(encrypted.starts_with("enc:"), "{target}: no marker");
            assert_eq!(
                sha256_hex(encrypted.as_bytes()),
                want["entry_sha256"].as_str().unwrap(),
                "{target}: the encrypted entry differs from Python's"
            );
            let secret = decrypt_api_key(encrypted, Some(&key_path)).unwrap();
            assert!(!secret.is_empty(), "{target}: secret did not decrypt");
            assert_eq!(
                sha256_hex(secret.as_bytes()),
                want["secret_sha256"].as_str().unwrap(),
                "{target}: decrypted secret differs from Python's"
            );
            assert_eq!(
                secret.chars().count(),
                want["secret_len"].as_u64().unwrap() as usize
            );
        }
    }

    /// The strongest forward claim: `src/readmd_modules/crypto.py`'s own
    /// `store_credential` (native keychain branch disabled) wrote the vault, and
    /// Rust's `load_credential` must resolve it through the default key path.
    /// Fixture: `scratch/rust_parity/fernet_python_store_credential.py`.
    #[test]
    fn reads_credentials_stored_by_the_python_crypto_module() {
        let _guard = data_dir_guard();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../scratch/rust_parity/module_vault");
        if !(dir.join("expected.json").is_file() && dir.join("credentials.vault").is_file()) {
            eprintln!("module_vault fixtures absent; run fernet_python_store_credential.py first");
            return;
        }
        std::env::set_var("READMD_DATA_DIR", &dir);

        let expected: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("expected.json")).unwrap()).unwrap();
        assert_eq!(fs::read(dir.join("encryption.key")).unwrap().len(), 44);
        let meta = &expected["__meta__"];
        assert_eq!(meta["key_len"].as_u64(), Some(44));
        assert_eq!(meta["key_is_ascii_b64"].as_bool(), Some(true));
        assert_eq!(meta["vault_prefix_enc"].as_bool(), Some(true));
        assert_eq!(meta["vault_trailing_newline"].as_bool(), Some(true));

        let vault = vault_load().unwrap();
        for cred_id in ["cred:pymade0001", "cred:pymade0002"] {
            let want = &expected[cred_id];
            let target = want["target"].as_str().unwrap();
            assert_eq!(target, format!("ReadMD/{}", cred_id));
            assert!(
                vault.get(target).is_some(),
                "{cred_id}: Rust's _vault_load did not see Python's target key"
            );
            let secret = load_credential(cred_id).unwrap();
            assert!(!secret.is_empty(), "{cred_id}: nothing came back");
            assert_eq!(
                sha256_hex(secret.as_bytes()),
                want["secret_sha256"].as_str().unwrap(),
                "{cred_id}: different secret than Python read back"
            );
            assert_eq!(
                secret.chars().count(),
                want["secret_len"].as_u64().unwrap() as usize
            );
        }

        // `ai.py:334` reads a provider `api_key` straight through
        // `decrypt_api_key`, so that shape has to open too.
        let direct = &expected["__direct__"];
        let secret = decrypt_api_key(direct["prefixed"].as_str().unwrap(), None).unwrap();
        assert_eq!(
            sha256_hex(secret.as_bytes()),
            direct["secret_sha256"].as_str().unwrap(),
            "directly encrypted api_key differs"
        );
        assert_eq!(
            secret.chars().count(),
            direct["secret_len"].as_u64().unwrap() as usize
        );

        std::env::remove_var("READMD_DATA_DIR");
    }

    /// The other direction of the live check: a vault written by
    /// [`store_credential`] must be readable by `crypto.load_credential`.
    ///
    /// The native tier is switched off for the duration exactly like
    /// `fernet_python_store_credential.py:30-32` does on the Python side, so this
    /// keeps producing a *vault* fixture instead of writing into the machine's
    /// credential store.
    #[test]
    fn exports_rust_vault_for_python_to_read() {
        let _guard = data_dir_guard();
        with_native_tier_disabled(|| {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../scratch/rust_parity/rust_vault");
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            std::env::set_var("READMD_DATA_DIR", &dir);

            let cases = [
                ("cred:rustmade0001", "sk-made-by-the-rust-kernel"),
                (
                    "cred:rustmade0002",
                    "sk-\u{6765}\u{81ea}rust\u{7684}\u{5bc6}\u{94a5}-\u{1f511}",
                ),
            ];
            let mut recorded = serde_json::Map::new();
            for (cred_id, secret) in cases {
                assert_eq!(
                    store_credential(cred_id, secret).unwrap(),
                    StoreBackend::EncryptedVault
                );
                assert_eq!(
                    load_credential(cred_id).unwrap(),
                    secret,
                    "Rust must read its own vault"
                );
                recorded.insert(
                    cred_id.to_string(),
                    serde_json::json!({
                        "target": validate_credential_id(cred_id).unwrap(),
                        "secret_sha256": sha256_hex(secret.as_bytes()),
                        "secret_len": secret.chars().count(),
                    }),
                );
            }
            std::env::remove_var("READMD_DATA_DIR");

            assert!(dir.join("encryption.key").is_file());
            assert!(dir.join("credentials.vault").is_file());
            fs::write(
                dir.join("expected.json"),
                serde_json::to_vec_pretty(&serde_json::Value::Object(recorded)).unwrap(),
            )
            .unwrap();
        });
    }

    /// `store_credential` / `load_credential` / `delete_credential` over the
    /// vault file, using a Python-generated key file so the whole chain is the
    /// same bytes the legacy app would read.
    ///
    /// Vault tier only: the native tier is switched off so this exercises the
    /// fallback branch rather than the Windows Credential Manager.
    #[test]
    fn vault_roundtrip_uses_python_key_file() {
        let _guard = data_dir_guard();
        with_native_tier_disabled(|| {
            let tmp = tempfile::tempdir().unwrap();
            std::env::set_var("READMD_DATA_DIR", tmp.path());
            fs::write(tmp.path().join("encryption.key"), VECTORS[3].1.as_bytes()).unwrap();

            let cred_id = "cred:test_cred_001";
            let secret = "sk-测试-credential-value";
            assert_eq!(
                store_credential(cred_id, secret).unwrap(),
                StoreBackend::EncryptedVault
            );

            // The vault file itself is one `enc:` line, like `_vault_write`.
            let vault = fs::read(tmp.path().join("credentials.vault")).unwrap();
            assert!(vault.starts_with(b"enc:"), "vault must hold a Fernet token");
            assert_eq!(
                vault.last(),
                Some(&b'\n'),
                "and exactly one trailing newline"
            );
            assert!(
                !String::from_utf8_lossy(&vault).contains("sk-"),
                "the secret must not be recoverable from the file bytes"
            );
            assert!(!tmp.path().join("credentials.vault.tmp").exists());

            assert_eq!(load_credential(cred_id).unwrap(), secret);
            assert_eq!(load_credential("cred:never_stored_01").unwrap(), "");

            delete_credential(cred_id).unwrap();
            assert_eq!(load_credential(cred_id).unwrap(), "");
            // `delete_credential` rewrites the emptied dict rather than unlinking.
            assert!(tmp.path().join("credentials.vault").is_file());

            // Invalid ids are refused the way `_credential_target` raises ValueError.
            assert!(store_credential("ReadMD/cred:abcd1234", "s").is_err());
            assert!(store_credential("cred:short", "secret").is_err());
            assert!(store_credential("cred:abcd1234", "").is_err());
            assert_eq!(load_credential("").unwrap(), "");
            assert_eq!(delete_credential("").unwrap(), ());

            std::env::remove_var("READMD_DATA_DIR");
        });
    }

    /// `crypto.store_credential` → `'native'` (`crypto.py:212-213`) is the tier
    /// this crate used to be missing.  When the OS accepts the record, the vault
    /// must not be touched at all, and `load_credential` must resolve through the
    /// OS store (`crypto.py:224-225`) rather than the vault branch.
    #[cfg(windows)]
    #[test]
    fn native_tier_wins_and_the_vault_is_never_written() {
        let _guard = data_dir_guard();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("READMD_DATA_DIR", tmp.path());

        let cred_id = "cred:nativetier01";
        let secret = "sk-原生-native-tier-密钥-🔑";
        let backend = store_credential(cred_id, secret).unwrap();
        if backend == StoreBackend::EncryptedVault {
            // `CredWriteW` was refused (locked-down policy, no credential
            // service).  That is Python's `except Exception` path too, so the
            // ladder itself still held — nothing to assert about the OS store.
            eprintln!("native tier unavailable on this host; ladder fell back to vault");
            delete_credential(cred_id).unwrap();
            std::env::remove_var("READMD_DATA_DIR");
            return;
        }

        assert_eq!(backend, StoreBackend::Native);
        assert!(
            !tmp.path().join("credentials.vault").is_file(),
            "Python writes the vault only when _native_store returns false"
        );
        assert!(
            !tmp.path().join("encryption.key").is_file(),
            "so the vault key must not be created either"
        );

        assert_eq!(native_load(cred_id).unwrap(), secret);
        assert_eq!(load_credential(cred_id).unwrap(), secret);

        // `crypto.py:235` calls `_native_delete` and discards its result.
        delete_credential(cred_id).unwrap();
        assert_eq!(native_load(cred_id).unwrap(), "");
        assert_eq!(load_credential(cred_id).unwrap(), "");

        std::env::remove_var("READMD_DATA_DIR");
    }

    /// A value the native tier *refuses* must still be persisted encrypted — the
    /// "silently swallow" failure mode.  The Credential Manager caps the blob at
    /// `CRED_MAX_CREDENTIAL_BLOB_SIZE` (`wincred.h:448`, `5 * 512` = 2560 bytes,
    /// confirmed by `scratch/rust_parity/_crypto_native_ffi_probe.rs`: 2560 writes
    /// Ok, 2561 answers Win32 error 1783).  pywin32 raises there and
    /// `crypto._native_store` swallows it, so both engines land in the vault.
    #[cfg(windows)]
    #[test]
    fn secret_over_the_native_blob_ceiling_falls_through_encrypted() {
        let _guard = data_dir_guard();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("READMD_DATA_DIR", tmp.path());
        fs::write(tmp.path().join("encryption.key"), VECTORS[3].1.as_bytes()).unwrap();

        let cred_id = "cred:nativetier02";
        let secret = "测".repeat(1000); // 3000 UTF-8 bytes > 2560
        assert_eq!(
            store_credential(cred_id, &secret).unwrap(),
            StoreBackend::EncryptedVault
        );
        assert_eq!(load_credential(cred_id).unwrap(), secret);
        assert!(
            !String::from_utf8_lossy(
                &fs::read(tmp.path().join("credentials.vault")).unwrap()
            )
            .contains('测'),
            "the fallback must still be ciphertext, never the secret in the clear"
        );

        delete_credential(cred_id).unwrap();
        assert_eq!(load_credential(cred_id).unwrap(), "");
        std::env::remove_var("READMD_DATA_DIR");
    }

    /// macOS and Linux reach their keychains through external programs
    /// (`crypto.py:152-163`, `crypto.py:177-186`), which the kernel will not
    /// spawn.  The honest answer is "tier unavailable, fall through" — the value
    /// still gets persisted, encrypted, and never in the clear.
    #[cfg(not(windows))]
    #[test]
    fn native_tier_reports_unavailable_and_the_vault_carries_it() {
        let _guard = data_dir_guard();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("READMD_DATA_DIR", tmp.path());
        fs::write(tmp.path().join("encryption.key"), VECTORS[3].1.as_bytes()).unwrap();

        let cred_id = "cred:nativetier03";
        let secret = "sk-native-tier-fallback";
        assert_eq!(native_store(cred_id, secret).unwrap(), false);
        assert_eq!(
            store_credential(cred_id, secret).unwrap(),
            StoreBackend::EncryptedVault
        );
        assert_eq!(load_credential(cred_id).unwrap(), secret);
        let vault = fs::read(tmp.path().join("credentials.vault")).unwrap();
        assert!(vault.starts_with(b"enc:"), "the value must be encrypted");
        assert!(
            !String::from_utf8_lossy(&vault).contains("sk-native-tier-fallback"),
            "and never stored in the clear"
        );

        delete_credential(cred_id).unwrap();
        assert_eq!(load_credential(cred_id).unwrap(), "");
        std::env::remove_var("READMD_DATA_DIR");
    }

    #[test]
    fn credential_id_gate_mirrors_python_regex() {
        assert_eq!(
            validate_credential_id("  cred:abcd1234  ").unwrap(),
            "ReadMD/cred:abcd1234"
        );
        assert_eq!(
            validate_credential_id("cred:A0z_-9876543210").unwrap(),
            "ReadMD/cred:A0z_-9876543210"
        );
        assert_eq!(
            validate_credential_id(&format!("cred:{}", "x".repeat(128))).unwrap(),
            format!("ReadMD/cred:{}", "x".repeat(128))
        );
        for bad in [
            "",
            "cred:",
            "cred:abcdefg", // 7 chars: below the {8,128} floor
            &format!("cred:{}", "x".repeat(129)),
            "abcd1234",
            "cred:has space",
            "cred:unicode_密钥",
            "CRED:abcd1234",
        ] {
            assert!(
                validate_credential_id(bad).is_err(),
                "python's fullmatch rejects {bad:?}"
            );
        }
    }

    #[test]
    fn fernet_roundtrip_over_random_plaintexts() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rt.key");
        for len in [1usize, 15, 16, 17, 31, 32, 48, 200, 1024] {
            let value = "a".repeat(len) + &"字".repeat(len % 3);
            let token = encrypt_api_key(&value, Some(&path)).unwrap();
            assert_eq!(
                decrypt_api_key(&token, Some(&path)).unwrap(),
                value,
                "len {len}"
            );
        }
    }
}
