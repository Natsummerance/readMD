# Production Deployment

## Production target

- Canonical site: `https://readmd.asia/`
- Hosting: GitHub Pages on the free plan
- Domain strategy: Cloudflare is authoritative for a dedicated subdomain of the existing active `syminu.online` zone. No new domain purchase is required.

## Active release path

Merging an approved website change into `main` runs `.github/workflows/website-github-pages.yml`. The workflow rebuilds `website/dist`, runs `validate_website.py --release`, uploads the artifact, and deploys GitHub Pages. The custom domain is declared by `website/public/CNAME`.

The active DNS-only record is:

```text
readmd.asia. CNAME Natsummerance.github.io.
```

Do not use the existing `readmd.syminu.online` or `www.syminu.online` records; both already point to other services. GitHub Pages has issued a certificate, and HTTPS enforcement is enabled in the Pages configuration.

## Local release check

Run from `website/`:

```powershell
npm run verify:release
```

This builds `dist/` and runs the GEO, approval, rights, security-header, structured-data, and release-build checks.

## Security headers

GitHub Pages does not process the `website/public/_headers` file, so those headers are not emitted by the current DNS-only origin. Applying the policy at the Cloudflare edge requires either:

- Zone > Transform Rules > Edit permission, followed by an HTTP response header transform scoped to `http.host eq "readmd.asia"`; or
- migrating the origin to Cloudflare Workers/Pages, where `_headers` can be honored directly.

Until one of those changes is made, do not claim that CSP, HSTS, or frame-protection headers are live. The file remains the canonical security policy for the future Cloudflare-native hosting migration.

## Future direct Cloudflare hosting

Cloudflare Workers remains available as a later hosting migration. Its deployment workflow is manual-only until the token is upgraded. The current token can identify the account, read zones, and edit DNS, but Cloudflare rejects Worker/Pages writes with authentication error `10000`. To enable that path, create a token with:

- Account > Workers Scripts > Edit
- Account > Account Settings > Edit (only needed the first time a workers.dev subdomain is registered)
- Zone > Workers Routes > Edit for `syminu.online`
- Zone > DNS > Edit for `syminu.online` (needed when Cloudflare creates the custom-domain record)

Then run:

```powershell
npx wrangler deploy --config wrangler.worker.jsonc
```

Afterward, change the `readmd.asia` CNAME target from `Natsummerance.github.io` to the Cloudflare custom-domain target and re-run the site validator.

## Historical Cloudflare blocker

Cloudflare rejected four write operations with the current token:

1. Creating the Pages project `readmd`.
2. Reading or registering an account-level workers.dev subdomain.
3. Uploading Worker static assets for `readmd-site`.
4. Completing the custom-domain Worker deployment.

No existing DNS record, website, or process was changed. See `showcase/reports/cloudflare_deployment_readiness.json` for exact evidence.
