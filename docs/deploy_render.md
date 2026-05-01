# Deploy to Render (Free Tier)

How to deploy the rollup server to Render so the WASM playground talks to a real, public backend instead of `localhost:3000`.

## Why Render

Free hosting tiers compared:

| Provider | Persistent disk | Credit card | Cold start |
|----------|-----------------|-------------|-----------|
| **Render** | No (free tier) | **No** | 30-60s |
| Fly.io | Yes | Required (after $5/7-day trial) | 1-2s |
| Shuttle | Limited | No | 5-10s |
| Railway | Yes | Required | < 1s |

For a learning/demo project that prioritizes "no credit card", Render wins. The trade-off is no persistent disk on free tier — sled state is lost on cold start. We handle that by re-seeding default accounts on startup.

## What's Already Set Up

This repo includes:

- **`Dockerfile`** — multi-stage build, ~30MB final image
- **`.dockerignore`** — excludes target/, web/, docs/, etc. for faster build
- **`PORT` env support** — `main.rs` reads `PORT` env var (Render assigns one dynamically)
- **`DATA_DIR` env support** — sled storage path is configurable

## Deploy Steps

### 1. Sign up at Render
[render.com](https://render.com), no credit card required.

### 2. Connect GitHub repo
- Dashboard → **New** → **Web Service**
- Connect your GitHub account
- Select `egpivo/rust-zkp`

### 3. Configure
- **Name**: `rust-zkp` (or anything; this becomes part of your URL)
- **Region**: Oregon / Frankfurt / Singapore (closest to you)
- **Branch**: `main`
- **Runtime**: **Docker** (auto-detected from `Dockerfile`)
- **Plan**: **Free**
- **Auto-Deploy**: Yes (redeploys on every push to main)

Click **Create Web Service**.

### 4. Wait for first build (5-10 min)
Render runs:
1. `docker build .`
2. Boots a container
3. Maps your service to a public URL

You get a URL like `https://rust-zkp.onrender.com`.

### 5. Verify
```bash
curl https://rust-zkp.onrender.com/health
# expect: ok

curl https://rust-zkp.onrender.com/accounts
# expect: JSON array with seeded accounts
```

### 6. Update WASM playground
Edit `web/src/pages/demos/send.astro`, `accounts.astro`:

```astro
<input id="server" value="https://rust-zkp.onrender.com">
```

Push, and GitHub Pages auto-deploys with the new default URL. Users hitting your playground will talk to your hosted server out of the box.

## What to Expect

- **Cold start**: 30-60s (free tier scales to zero after 15min idle)
- **State**: re-seeded on each cold start (no persistent disk)
- **Logs**: Render dashboard → service → Logs tab
- **CORS**: already enabled in `main.rs`, all browsers can hit it
- **HTTPS**: free, auto-provisioned via Let's Encrypt
- **Custom domain**: free on Render (paid features start at custom regions / static IPs)

## Limits to Know

| Resource | Free tier |
|----------|-----------|
| RAM | 512 MB |
| CPU | 0.1 shared |
| Bandwidth | 100 GB/mo |
| Build time | 90 min/mo (Docker builds) |
| Idle timeout | 15 min → scales to zero |
| Cold wake | 30-60s |

For a low-traffic demo, this is plenty. If you outgrow it: move to Fly.io ($2-5/mo with persistent disk) or upgrade Render to $7/mo for always-on.

## Common Pitfalls

### 1. "Failed to fetch" from browser
Likely CORS — but you've already configured `Any` origin, so this should work. If you hit issues:
- Open browser DevTools → Network tab
- Check the failing request's Response Headers
- Make sure `access-control-allow-origin: *` is present

### 2. App is "spinning down" / slow first request
Cold start. Hit `/health` to wake it before testing other endpoints.

### 3. Logs show panic
`docker run` locally first to test:
```bash
docker build -t zkp .
docker run -p 3000:3000 zkp
curl http://localhost:3000/health
```

### 4. Build fails on Render
Common reasons:
- Cargo.lock missing (commit it)
- Dependencies need system libs (the Dockerfile installs `pkg-config libssl-dev` already)
- Free tier ran out of build minutes (check dashboard)

## Rollback

If a deploy breaks:
- Render dashboard → Deploys tab
- Click any past deploy → **Redeploy**

That's it.

## Why No Persistent Disk on Free Tier

Render's free tier doesn't include persistent volumes. The container's filesystem is ephemeral — anything written disappears on restart.

For our demo this means:
- Accounts created via `POST /accounts` survive within a session
- After 15min idle → container dies → state wiped
- Next request → cold start → re-seeds the 4 default accounts

If you need persistence on Render free, you'd either:
- Use Render PostgreSQL (free 90 days, then $7/mo)
- Upgrade to Render's $7/mo tier with disk
- Move to Fly.io with persistent volumes

For learning purposes, ephemeral state is acceptable — the **architecture** is real, just the data isn't durable.
