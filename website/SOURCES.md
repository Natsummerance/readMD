# Source Selection And Usage

## GitHub search evidence (2026-08-24)

The implementation uses Tailwind CSS v4 as the production CSS build system. A live GitHub repository search for `css framework` ranked `tailwindlabs/tailwindcss` at **97,306 stars**, behind Bootstrap at **174,642 stars**. Tailwind was selected because the site needs a small utility-driven stylesheet, deterministic minified output, and a Cloudflare Pages build without shipping unused JavaScript components.

The four requested research repositories were cloned into the ignored sandbox path `sandbox/vendor/` on commit `HEAD` at the time of review:

- `every-app/open-seo`: used its workflow taxonomy—keyword intent, site audit, competitor insight, backlinks, technical audit, and AI visibility—to structure the promotion checklist rather than copying UI code.
- `AgriciDaniel/claude-seo`: applied its GEO rubric for AI crawler access, passage-level citability, entity signals, SSR accessibility, and platform-specific visibility.
- `zubair-trabzada/geo-seo-claude`: followed its weighted GEO scoring model and its `llms.txt` / `llms-full.txt` generation contract.
- `AgriciDaniel/claude-blog`: reused its release discipline: prose linting, consistency checks, evidence-backed claims, no unverified performance promises, and explicit citation guidance.

## Product content rules

- Every answer states the conclusion before supporting detail.
- Numeric limits are stated with their source boundary: approximately 8,000 lines or 500 KB.
- No unverifiable speed, ranking, adoption, or AI-citation guarantee appears.
- Download safety points to GitHub Releases and SHA256SUMS.txt instead of making an unsupported “always safe” claim.
- The visual language is original. It does not copy Apple text, images, logos, page source, proprietary assets, or brand identity.
