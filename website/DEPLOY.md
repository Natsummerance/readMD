# Production Deployment

## Production target

- Canonical site: `https://app.syminu.online/`
- Hosting: GitHub Pages on the free plan
- Domain strategy: Cloudflare manages a dedicated subdomain of the existing active `syminu.online` zone. No new domain purchase is required.

## Active release path

Merging an approved website change into `main` runs `.github/workflows/website-github-pages.yml`. The workflow rebuilds `website/dist`, runs `validate_website.py --release`, uploads the artifact, and deploys GitHub Pages. The custom domain is declared by `website/public/CNAME`.

The required DNS-only record is:

```text
app.syminu.online. CNAME Natsummerance.github.io.
```

Do not use the existing `readmd.syminu.online` or `www.syminu.online` records; both already point to other services. After GitHub issues a certificate, HTTPS enforcement can be enabled through the Pages API.

## Local release check

Run from `website/`:

```powershell
npm run verify:release
```

This builds `dist/` and runs the GEO, approval, rights, security-header, structured-data, and release-build checks.

## Future direct Cloudflare hosting

Cloudflare Workers remains available as a later hosting migration. The current token can identify the account, read zones, and edit DNS, but Cloudflare rejects Worker/Pages writes with authentication error `10000`. To enable that path, create a token with:

- Account > Workers Scripts > Edit
- Account > Account Settings > Edit (only needed the first time a workers.dev subdomain is registered)
- Zone > Workers Routes > Edit for `syminu.online`
- Zone > DNS > Edit for `syminu.online` (needed when Cloudflare creates the custom-domain record)

Then run:

```powershell
npx wrangler deploy --config wrangler.worker.jsonc
```

Afterward, change the `app.syminu.online` CNAME target from `Natsummerance.github.io` to the Cloudflare custom-domain target and re-run the site validator.

## Historical Cloudflare blocker

Cloudflare rejected four write operations with the current token:

1. Creating the Pages project `readmd`.
2. Reading or registering an account-level workers.dev subdomain.
3. Uploading Worker static assets for `readmd-site`.
4. Completing the custom-domain Worker deployment.

No existing DNS record, website, or process was changed. See `showcase/reports/cloudflare_deployment_readiness.json` for exact evidence.
