import subprocess
import time
import json
import urllib.request
import urllib.parse
import sys
from pathlib import Path
sys.path.insert(0, ".")
from playwright.sync_api import sync_playwright
from tests.browser_socratic_audit import ReadMDServer, BASE_URL

def main():
    # 1. Generate sample_diagrams.md
    from tests.create_diagram_fixture import test_file
    
    server = ReadMDServer()
    server.start()

    console_logs = []
    page_errors = []

    try:
        with sync_playwright() as p:
            browser = p.chromium.launch(headless=True)
            page = browser.new_page()
            
            page.on("console", lambda msg: console_logs.append(f"[{msg.type}] {msg.text}"))
            page.on("pageerror", lambda err: page_errors.append(str(err)))

            # Open document
            abs_path = str(test_file.resolve())
            url = f"{BASE_URL}/?file={urllib.parse.quote(abs_path)}"
            print(f"Loading URL: {url}")
            page.goto(url)
            page.wait_for_selector("#content .markdown-body", timeout=10000)

            # Wait for diagrams to settle
            print("Waiting for diagrams to render...")
            time.sleep(5)

            cards = page.locator(".diagram-card").all()
            print(f"Found {len(cards)} diagram cards.")

            for i, card in enumerate(cards):
                engine = card.get_attribute("data-diagram-engine")
                preview = card.locator(".diagram-preview")
                html = preview.inner_html()
                has_fallback = "diagram-fallback" in html or "diagram_render_failed" in html
                is_loading = "diagram-loading" in html
                has_output_svg = bool(preview.locator("svg:not(.tb-ic)").count())
                has_canvas = bool(preview.locator("canvas").count())
                
                if has_fallback:
                    status = "FALLBACK / ERROR"
                elif is_loading:
                    status = "STUCK IN LOADING"
                elif has_output_svg:
                    status = "OK (SVG rendered)"
                elif has_canvas:
                    status = "OK (Canvas rendered)"
                else:
                    status = f"UNKNOWN ({html[:50]})"

                print(f"  [{i+1}] Engine: {engine:12s} -> Status: {status}")
                if "OK" not in status:
                    snippet = html[:200].replace('\n', ' ')
                    print(f"       Preview snippet: {snippet}")

            # Test PlantUML Allow Online Render button
            allow_btn = page.locator(".diagram-allow-remote-btn")
            if allow_btn.count() > 0:
                print("\nClicking PlantUML 'Allow Online Render' button...")
                allow_btn.first.click()
                # Wait for remote fetch
                time.sleep(4)
                plantuml_card = page.locator('.diagram-card[data-diagram-engine="plantuml"]')
                plantuml_html = plantuml_card.locator(".diagram-preview").inner_html()
                plantuml_svg = bool(plantuml_card.locator(".diagram-preview svg:not(.tb-ic)").count())
                print(f"PlantUML after allowing remote render: {'OK (SVG rendered)' if plantuml_svg else 'FAILED'}")
                # Check indicator badge
                has_badge = bool(plantuml_card.locator(".diagram-network-indicator").count())
                badge_text = plantuml_card.locator(".diagram-network-indicator").text_content() if has_badge else ""
                print(f"PlantUML network badge present: {has_badge} ({badge_text.strip()})")

            # Take screenshot of the rendered diagrams
            screenshots_dir = Path("tests/audit_screenshots")
            screenshots_dir.mkdir(parents=True, exist_ok=True)
            shot_path = screenshots_dir / "diagrams_test_result.png"
            page.screenshot(path=str(shot_path), full_page=True)
            print(f"Overview screenshot saved to {shot_path}")

            # Capture individual screenshots for each diagram card
            for i, card in enumerate(page.locator(".diagram-card").all()):
                engine_name = card.get_attribute("data-diagram-engine") or f"card_{i+1}"
                card.scroll_into_view_if_needed()
                time.sleep(0.2)
                card_shot = screenshots_dir / f"diagram_{i+1}_{engine_name}.png"
                card.screenshot(path=str(card_shot))
                print(f"Saved card screenshot: {card_shot.name}")

            print("\n--- Console logs ---")
            for log in console_logs:
                if any(x in log for x in ["diagram", "error", "warn", "Error", "Warn", "failed"]):
                    print("  ", log)

            print("\n--- Page errors ---")
            for err in page_errors:
                print("  ", err)

            browser.close()
    finally:
        server.stop()

if __name__ == "__main__":
    main()
