# Cloudflare Deployment

## Production target

- Canonical site: `https://readmd.syminu.online/`
- Worker name: `readmd-site`
- Hosting: Cloudflare Workers static assets on the free plan
- Domain strategy: add a free `readmd` subdomain to the existing active Cloudflare zone `syminu.online`. No new domain purchase and no change to the existing apex site are required.

## One-command release check and deploy

Run from `website/`:

```powershell
npm run deploy
```

The command performs this sequence:

1. Clean and rebuild `dist/`.
2. Run the GEO, approval, rights, security-header, and release-build validator.
3. Deploy the verified assets with Wrangler using `wrangler.worker.jsonc`.

## Required API-token permissions

The current `CLOUDFLARE_API_TOKEN` can identify the account and read zones, but Cloudflare rejects writes with authentication error `10000`. To deploy without opening the dashboard session each time, create a token with:

- Account > Workers Scripts > Edit
- Account > Account Settings > Edit (only needed the first time a workers.dev subdomain is registered)
- Zone > Workers Routes > Edit for `syminu.online`
- Zone > DNS > Edit for `syminu.online` (needed when Cloudflare creates the custom-domain record)

For the alternative Pages path instead of Workers, use:

- Account > Cloudflare Pages > Edit

Then run:

```powershell
npx wrangler pages project create readmd --production-branch=main
npx wrangler pages deploy dist --project-name=readmd --branch=main
```

## Current blocker

Cloudflare rejected four write operations with the current token:

1. Creating the Pages project `readmd`.
2. Reading or registering an account-level workers.dev subdomain.
3. Uploading Worker static assets for `readmd-site`.
4. Completing the custom-domain Worker deployment.

No existing DNS record, website, or process was changed. See `showcase/reports/cloudflare_deployment_readiness.json` for exact evidence.
