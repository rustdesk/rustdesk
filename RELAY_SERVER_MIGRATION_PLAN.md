# RustDesk Relay & ID Server Cloud Migration Plan

This document outlines the step-by-step plan for migrating the local Docker-based RustDesk servers (`hbbs` and `hbbr`) to a 24/7 online Cloud VPS.

---

## 📋 1. Target Infrastructure Comparison

To run RustDesk securely, we require a server with:
* 1–2 vCPUs, 2 GB RAM, 20+ GB SSD.
* Static IPv4 Address.
* Open Ports: **TCP** 21115-21119, **UDP** 21116.

| Provider | Specs | Monthly Cost | Pros | Cons |
| :--- | :--- | :--- | :--- | :--- |
| **Hetzner Cloud** *(CPX11)* | 2 vCPU / 2 GB RAM | ~€4.39 | Cheapest, lowest latency to NL (<15ms), 20 TB bandwidth. | Self-managed Linux configuration. |
| **Elestio** *(Managed)* | 1 vCPU / 2 GB RAM | ~$8.50 | Pre-configured scripts exist in repo, auto-backups, managed OS. | Higher cost. |
| **DigitalOcean** | 1 vCPU / 1 GB RAM | $6.00 | Beginner-friendly panel, Amsterdam datacenter. | Higher price for 2 GB RAM ($12). |

**Recommended Choice:** **Hetzner Cloud** for cost/performance, or **Elestio** for easy maintenance using our existing scripts.

---

## 🔑 2. The Core Rule: Encryption Key Migration

Repacked clients expect the current server public key:
`VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=`

> [!WARNING]
> Do **not** let the new server auto-generate new keys. If it does, all deployed client devices will fail to connect. You must copy the existing keys from the local desktop's Docker directory to the new server.

**Files to Migrate:**
* `id_ed25519` (Private key)
* `id_ed25519.pub` (Public key)

---

## 🚀 3. Step-by-Step Migration Plan

### Phase 1: DNS & VPS Setup
1. **Purchase VPS:** Spin up a new instance on your chosen provider (Ubuntu 22.04 LTS recommended).
2. **Configure DNS:** Update the A-records for `hbbs.cislink.nl` and `hbbr.cislink.nl` to point to the new VPS public IP.
3. **Firewall Rules:** Open the following ports on the VPS firewall:
   * **TCP:** `21115`, `21116`, `21117`, `21118`, `21119`
   * **UDP:** `21116`

### Phase 2: Key Migration
1. Locate the local key files on your desktop machine (usually in `D:\Rustdesk\data` or inside your local Docker volumes).
2. Copy them to the server before booting up the docker containers:
   ```bash
   scp -P <SSH_Port> ./data/id_ed25519* root@<VPS_IP>:/opt/rustdesk/data/
   ```

### Phase 3: Docker-Compose Deployment
1. SSH into the cloud server:
   ```bash
   ssh root@<VPS_IP> -p <SSH_Port>
   ```
2. Install Docker and Docker Compose:
   ```bash
   sudo apt update && sudo apt install -y docker.io docker-compose
   ```
3. Create `/opt/rustdesk/docker-compose.yml` with the following configuration:
   ```yaml
   version: '3'
   services:
     hbbs:
       container_name: hbbs
       image: rustdesk/rustdesk-server:latest
       command: hbbs -r hbbr.cislink.nl:21117
       volumes:
         - ./data:/root
       ports:
         - 21115:21115
         - 21116:21116
         - 21116:21116/udp
         - 21118:21118
       restart: unless-stopped

     hbbr:
       container_name: hbbr
       image: rustdesk/rustdesk-server:latest
       command: hbbr
       volumes:
         - ./data:/root
       ports:
         - 21117:21117
         - 21119:21119
       restart: unless-stopped
   ```
4. Start the service:
   ```bash
   cd /opt/rustdesk
   docker-compose up -d
   ```

### Phase 4: Verification & Testing
1. **Check Logs:** Verify `hbbs` and `hbbr` are running smoothly:
   ```bash
   docker-compose logs -f
   ```
2. **Verify Key:** Ensure the generated key matches your original public key:
   ```bash
   cat ./data/id_ed25519.pub
   ```
3. **Client Test:** Launch your repacked RustDesk client and verify it successfully connects to `hbbs.cislink.nl` without warnings.

---

## ↩️ 4. Rollback Plan

If anything fails or connection issues arise during setup:
1. Revert `hbbs.cislink.nl` and `hbbr.cislink.nl` DNS A-records back to your desktop public IP (or DDNS domain).
2. Start the local desktop docker containers back up.
3. Deployed clients will automatically reconnect to the local instance within minutes.
