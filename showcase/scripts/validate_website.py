from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlparse


ROOT = Path(__file__).resolve().parents[2]
SITE = ROOT / "website"
PUBLIC = SITE / "public"

LANGUAGES = {
    "en": {"path": PUBLIC / "index.html", "canonical": "https://app.syminu.online/", "full": PUBLIC / "llms-full.txt"},
    "zh-CN": {"path": PUBLIC / "zh-cn" / "index.html", "canonical": "https://app.syminu.online/zh-cn/", "full": PUBLIC / "zh-cn" / "llms-full.txt"},
    "zh-TW": {"path": PUBLIC / "zh-tw" / "index.html", "canonical": "https://app.syminu.online/zh-tw/", "full": PUBLIC / "zh-tw" / "llms-full.txt"},
    "ja": {"path": PUBLIC / "ja" / "index.html", "canonical": "https://app.syminu.online/ja/", "full": PUBLIC / "ja" / "llms-full.txt"},
}

INTENT_PAGES = {
    "en": {"path": PUBLIC / "workflows" / "index.html", "canonical": "https://app.syminu.online/workflows/"},
    "zh-CN": {"path": PUBLIC / "zh-cn" / "workflows" / "index.html", "canonical": "https://app.syminu.online/zh-cn/workflows/"},
    "zh-TW": {"path": PUBLIC / "zh-tw" / "workflows" / "index.html", "canonical": "https://app.syminu.online/zh-tw/workflows/"},
    "ja": {"path": PUBLIC / "ja" / "workflows" / "index.html", "canonical": "https://app.syminu.online/ja/workflows/"},
}

DOWNLOAD_PAGES = {
    "en": {"path": PUBLIC / "download" / "index.html", "canonical": "https://app.syminu.online/download/"},
    "zh-CN": {"path": PUBLIC / "zh-cn" / "download" / "index.html", "canonical": "https://app.syminu.online/zh-cn/download/"},
    "zh-TW": {"path": PUBLIC / "zh-tw" / "download" / "index.html", "canonical": "https://app.syminu.online/zh-tw/download/"},
    "ja": {"path": PUBLIC / "ja" / "download" / "index.html", "canonical": "https://app.syminu.online/ja/download/"},
}

AI_CRAWLERS = ("GPTBot", "OAI-SearchBot", "ClaudeBot", "PerplexityBot")


class PageAudit(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.title = ""
        self.in_title = False
        self.metas: list[dict[str, str]] = []
        self.links: list[dict[str, str]] = []
        self.headings: list[tuple[str, str]] = []
        self.images: list[dict[str, str]] = []
        self.stylesheets: list[str] = []
        self.current_heading = ""

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        normalized = {key: value or "" for key, value in attrs}
        if tag == "title":
            self.in_title = True
        elif tag == "meta":
            self.metas.append(normalized)
        elif tag == "link":
            self.links.append(normalized)
            if normalized.get("rel") == "stylesheet":
                self.stylesheets.append(normalized.get("href", ""))
        elif tag in {"h1", "h2", "h3"}:
            self.current_heading = tag
        elif tag == "img":
            self.images.append(normalized)

    def handle_data(self, data: str) -> None:
        if self.in_title:
            self.title += data.strip()
        if self.current_heading:
            self.headings.append((self.current_heading, " ".join(data.split())))

    def handle_endtag(self, tag: str) -> None:
        if tag == "title":
            self.in_title = False
        elif tag in {"h1", "h2", "h3"}:
            self.current_heading = ""


def audit_page(path: Path, canonical: str) -> list[str]:
    errors: list[str] = []
    content = path.read_text(encoding="utf-8")
    audit = PageAudit()
    audit.feed(content)
    if len(audit.title) < 15 or "ReadMD" not in audit.title:
        errors.append(f"{path}: missing descriptive ReadMD title")
    descriptions = [item for item in audit.metas if item.get("name") == "description"]
    if len(descriptions) != 1 or len(descriptions[0].get("content", "")) < 80:
        errors.append(f"{path}: missing unique meta description of at least 80 characters")
    robots_directives = [item for item in audit.metas if item.get("name") == "robots"]
    if len(robots_directives) != 1 or robots_directives[0].get("content") != "index,follow,max-image-preview:large":
        errors.append(f"{path}: missing canonical index,follow,max-image-preview:large directive")
    og = {item.get("property"): item.get("content") for item in audit.metas if str(item.get("property", "")).startswith("og:")}
    for required in (
        "og:title",
        "og:description",
        "og:url",
        "og:image",
        "og:image:type",
        "og:image:alt",
    ):
        if len(og.get(required, "")) < 8:
            errors.append(f"{path}: missing complete {required}")
    if og.get("og:image:width") != "1440" or og.get("og:image:height") != "900":
        errors.append(f"{path}: Open Graph image must declare 1440x900")
    if og.get("og:url") != canonical:
        errors.append(f"{path}: og:url differs from canonical")
    twitter = {item.get("name"): item.get("content") for item in audit.metas if str(item.get("name", "")).startswith("twitter:")}
    for required in (
        "twitter:card",
        "twitter:title",
        "twitter:description",
        "twitter:image",
        "twitter:image:alt",
    ):
        if len(twitter.get(required, "")) < 8:
            errors.append(f"{path}: missing complete {required}")
    canonicals = [item for item in audit.links if item.get("rel") == "canonical"]
    if len(canonicals) != 1 or canonicals[0].get("href") != canonical:
        errors.append(f"{path}: canonical must be exactly {canonical}")
    hreflang = {item.get("hreflang") for item in audit.links if item.get("rel") == "alternate" and item.get("hreflang")}
    if hreflang != {"en", "zh-CN", "zh-TW", "ja", "x-default"}:
        errors.append(f"{path}: incomplete hreflang set: {sorted(item for item in hreflang if item)}")
    h1_values = [text for tag, text in audit.headings if tag == "h1" and text]
    if len(h1_values) != 1:
        errors.append(f"{path}: page must contain exactly one non-empty h1")
    for image in audit.images:
        if len(image.get("alt", "").strip()) < 10:
            errors.append(f"{path}: image lacks meaningful alt text: {image.get('src', '')}")
        if image.get("loading") == "eager" and image.get("fetchpriority") != "high":
            errors.append(f"{path}: eager hero image must declare fetchpriority=high")
    if "/assets/site.css" not in audit.stylesheets:
        errors.append(f"{path}: production stylesheet link is missing")
    if content.count("<picture>") != len(audit.images):
        errors.append(f"{path}: every product image must have a WebP picture fallback")
    if ".webp" not in content:
        errors.append(f"{path}: optimized WebP source is missing")
    if "https://github.com/Natsummerance/readMD/stargazers" not in content:
        errors.append(f"{path}: star call to action is missing")
    jsonld_match = re.search(r'(?s)<script type="application/ld\+json">(.*?)</script>', content)
    if not jsonld_match:
        errors.append(f"{path}: server-rendered JSON-LD is missing")
    else:
        try:
            graph = json.loads(jsonld_match.group(1)).get("@graph", [])
            types = {item.get("@type") for item in graph if isinstance(item, dict)}
            if not {"WebPage", "SoftwareApplication"} <= types:
                errors.append(f"{path}: JSON-LD lacks WebPage and SoftwareApplication")
            if not any(isinstance(item, dict) and "speakable" in item for item in graph):
                errors.append(f"{path}: JSON-LD lacks speakable definition")
        except (TypeError, ValueError, json.JSONDecodeError) as exc:
            errors.append(f"{path}: invalid JSON-LD: {exc}")
    return errors


def validate_llms(path: Path, *, minimum_absolute_links: int = 5) -> list[str]:
    errors: list[str] = []
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or not lines[0].startswith("# ReadMD"):
        errors.append(f"{path}: first line must be an H1 starting with ReadMD")
    if len(lines) < 2 or not lines[1].startswith(">"):
        errors.append(f"{path}: second line must be a blockquote description")
    if len(lines[1]) > 220:
        errors.append(f"{path}: description exceeds the compact llms.txt contract")
    absolute_links = re.findall(r"https://app\.syminu\.online(?:/[\w.-]+)*", path.read_text(encoding="utf-8"))
    if len(absolute_links) < minimum_absolute_links:
        errors.append(f"{path}: fewer than {minimum_absolute_links} absolute canonical entries")
    return errors


def validate_robots_and_sitemap() -> list[str]:
    errors: list[str] = []
    robots = (PUBLIC / "robots.txt").read_text(encoding="utf-8")
    for crawler in AI_CRAWLERS:
        pattern = f"User-agent: {crawler}\nAllow: /"
        if pattern not in robots:
            errors.append(f"robots.txt does not explicitly allow {crawler}")
    if "Sitemap: https://app.syminu.online/sitemap.xml" not in robots:
        errors.append("robots.txt omits canonical sitemap")
    sitemap = (PUBLIC / "sitemap.xml").read_text(encoding="utf-8")
    expected = {item["canonical"] for item in LANGUAGES.values()}
    expected.update(item["canonical"] for item in INTENT_PAGES.values())
    expected.update(item["canonical"] for item in DOWNLOAD_PAGES.values())
    actual = set(re.findall(r"<loc>(.*?)</loc>", sitemap))
    if actual != expected:
        errors.append(f"sitemap mismatch: missing={expected - actual}, extra={actual - expected}")
    return errors


def validate_language_crosslinks() -> list[str]:
    errors: list[str] = []
    for language, contract in LANGUAGES.items():
        html_text = contract["path"].read_text(encoding="utf-8")
        relative_full = contract["full"].relative_to(PUBLIC).as_posix()
        if f'href="/{relative_full}"' not in html_text:
            errors.append(f"{language} index does not link its own llms-full corpus")
        localized_index = contract["path"].parent / "llms.txt"
        if not localized_index.is_file():
            errors.append(f"{language} is missing llms.txt")
        else:
            localized_text = localized_index.read_text(encoding="utf-8")
            errors.extend(validate_llms(localized_index, minimum_absolute_links=3))
            if contract["canonical"] not in localized_text:
                errors.append(f"{language} llms.txt omits its localized homepage")
        errors.extend(validate_llms(contract["full"]))
    return errors


def validate_approval() -> list[str]:
    errors: list[str] = []
    approval_path = ROOT / "showcase" / "reports" / "website_approval.json"
    approval = json.loads(approval_path.read_text(encoding="utf-8"))
    minimum = float(approval["minimum_score"])
    rounds = approval.get("rounds", [])
    if len(rounds) < int(approval.get("required_rounds", 3)):
        errors.append("approval has fewer than three review rounds")
    for item in rounds:
        if item.get("status") != "approved" or float(item.get("score", 0)) < minimum:
            errors.append(f"approval round {item.get('round')} is not approved at or above {minimum}")
    meeting = approval.get("final_decision_meeting", {})
    if meeting.get("status") != "approved_for_staged_publication" or float(meeting.get("score", 0)) < minimum:
        errors.append("final decision meeting is not approved at or above threshold")
    if not all(meeting.get("conditions")):
        errors.append("final decision conditions are incomplete")
    return errors


def validate_rights() -> list[str]:
    errors: list[str] = []
    forbidden = ("apple.com", "1比1", "1:1 copy", "完全一致")
    for path in PUBLIC.rglob("*.html"):
        text = path.read_text(encoding="utf-8")
        for marker in forbidden:
            if marker in text:
                errors.append(f"{path}: forbidden clone/reference marker found: {marker}")
    return errors


def validate_security_headers() -> list[str]:
    errors: list[str] = []
    headers = (PUBLIC / "_headers").read_text(encoding="utf-8")
    required = (
        "Content-Security-Policy:",
        "Strict-Transport-Security:",
        "X-Content-Type-Options: nosniff",
        "X-Frame-Options: DENY",
        "Referrer-Policy:",
        "Permissions-Policy:",
    )
    for header in required:
        if header not in headers:
            errors.append(f"_headers omits security header: {header}")
    if "script-src 'self'" not in headers:
        errors.append("_headers CSP does not constrain scripts to self")
    root = (PUBLIC / "index.html").read_text(encoding="utf-8")
    match = re.search(r'(?s)<script type="application/ld\+json">(.*?)</script>', root)
    if match:
        digest = hashlib.sha256(match.group(1).encode("utf-8")).digest()
        expected_hash = "sha256-" + base64.b64encode(digest).decode("ascii")
        if expected_hash not in headers:
            errors.append(f"_headers CSP omits current JSON-LD hash: {expected_hash}")
    return errors


def validate_growth_homepages() -> list[str]:
    errors: list[str] = []
    for language, contract in LANGUAGES.items():
        content = contract["path"].read_text(encoding="utf-8")
        if 'rel="preload" as="image" href="/media/overview-reader.webp"' not in content:
            errors.append(f"{language}: hero WebP preload is missing")
        if 'id="share"' not in content:
            errors.append(f"{language}: share section is missing")
        for growth_signal in ("twitter.com/intent/tweet", "t.me/share/url", "linkedin.com/sharing/share-offsite"):
            if growth_signal not in content:
                errors.append(f"{language}: share network missing: {growth_signal}")
    return errors


def validate_special_page_internal_links() -> list[str]:
    errors: list[str] = []
    for language in LANGUAGES:
        workflow = INTENT_PAGES[language]
        download = DOWNLOAD_PAGES[language]
        for source, target_contract in (
            ("workflow", workflow),
            ("download", download),
        ):
            content = target_contract["path"].read_text(encoding="utf-8")
            target = download["canonical"] if source == "workflow" else workflow["canonical"]
            target_path = urlparse(target).path
            if f'href="{target_path}"' not in content:
                errors.append(f"{language} {source} page omits its sibling internal link")
            if '"@type":"BreadcrumbList"' not in content:
                errors.append(f"{language} {source} page omits BreadcrumbList structured data")
    return errors


def validate_release_build() -> list[str]:
    errors: list[str] = []
    dist = SITE / "dist"
    if not (dist / "assets" / "site.css").is_file():
        errors.append("dist/assets/site.css is missing")
    if not (dist / "index.html").is_file():
        errors.append("dist/index.html is missing")
    return errors


def validate_indexnow() -> list[str]:
    errors: list[str] = []
    key_files = list(PUBLIC.glob("[0-9a-f]" * 32 + ".txt"))
    if len(key_files) != 1:
        return ["IndexNow requires exactly one 32-character hexadecimal key file"]
    key_file = key_files[0]
    key = key_file.stem
    content = key_file.read_text(encoding="utf-8").strip()
    if content != key or len(key) != 32 or not re.fullmatch(r"[0-9a-f]{32}", key):
        errors.append("IndexNow key filename and contents do not match")
    if not (SITE / "tools" / "indexnow-submit.mjs").is_file():
        errors.append("IndexNow submission script is missing")
    if '"indexnow"' not in (SITE / "package.json").read_text(encoding="utf-8"):
        errors.append("package.json omits the IndexNow command")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate the staged ReadMD website.")
    parser.add_argument("--release", action="store_true", help="also require a completed dist build")
    args = parser.parse_args()
    errors: list[str] = []
    for language, contract in LANGUAGES.items():
        if not contract["path"].is_file():
            errors.append(f"missing {language} index")
            continue
        errors.extend(audit_page(contract["path"], contract["canonical"]))
    for language, contract in INTENT_PAGES.items():
        if not contract["path"].is_file():
            errors.append(f"missing {language} workflow page")
            continue
        errors.extend(audit_page(contract["path"], contract["canonical"]))
    for language, contract in DOWNLOAD_PAGES.items():
        if not contract["path"].is_file():
            errors.append(f"missing {language} download page")
            continue
        errors.extend(audit_page(contract["path"], contract["canonical"]))
    if not (PUBLIC / "llms.txt").is_file():
        errors.append("missing public/llms.txt")
    else:
        errors.extend(validate_llms(PUBLIC / "llms.txt"))
    errors.extend(validate_language_crosslinks())
    errors.extend(validate_robots_and_sitemap())
    errors.extend(validate_approval())
    errors.extend(validate_rights())
    errors.extend(validate_security_headers())
    errors.extend(validate_growth_homepages())
    errors.extend(validate_special_page_internal_links())
    errors.extend(validate_indexnow())
    if args.release:
        errors.extend(validate_release_build())
    if errors:
        print(json.dumps({"ok": False, "errors": errors}, ensure_ascii=False))
        return 1
    print(json.dumps({
        "ok": True,
        "languages": list(LANGUAGES),
        "intent_pages": list(INTENT_PAGES),
        "download_pages": list(DOWNLOAD_PAGES),
        "review_rounds": 3,
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
