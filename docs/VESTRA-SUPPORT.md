# Vestra Support

This document outlines the project overview, goals, workflows, and status of the Vestra Support project.

---

## Project Overview

**Vestra Support** is a custom remote support application tailored for Vestra Interactive's client ecosystem. It is built as a lightweight, branded downstream fork of the open-source [RustDesk](https://github.com/rustdesk/rustdesk) remote desktop software. The project includes customized desktop client binaries and a dedicated, self-hosted relay infrastructure.

Vestra Support simplifies connection setup for customers and technicians by eliminating external third-party dependencies (such as TeamViewer or official RustDesk public relays) and routing all support traffic through Vestra-managed infrastructure.

---

## Business Goals

1. **Security & Data Privacy**: Retain complete control over connection logs and transit data by routing connections through Vestra's private relay network.
2. **Simplified Branding & Trust**: Reduce customer confusion and security skepticism by presenting a clear, Vestra-branded application dialog instead of a generic remote access utility.
3. **Optimized Support Workflows**: Integrate remote connection controls directly into Vestra's existing management platforms and helpdesk portal.
4. **License & Cost Optimization**: Leverage robust open-source foundations (AGPL-3.0) to eliminate seat-based subscription fees associated with proprietary remote support software.

---

## Workflows

### 1. One-Time Support Workflow (Ad-hoc Session)

Designed for users requiring immediate, on-demand support who do not have pre-installed management agents:
1. **Initiation**: The customer contacts Vestra support (via email or support portal).
2. **Download**: The technician directs the customer to download the lightweight, pre-configured Vestra Support runner binary from the Vestra Support URL (`https://support.vestrainteractive.com/download`).
3. **Execution**: The customer runs the application (no installation or administrative privileges required on standard systems).
4. **Handshake**: The client automatically establishes a session with the Vestra `hbbs` (ID/rendezvous) server and displays a unique, pre-generated ID and password.
5. **Connection**: The customer provides the ID and password to the Vestra technician, who connects using the technician's console.
6. **Termination**: Once the session is closed, the client application terminates completely, leaving no background services running.

### 2. Future Managed-Services Workflow (Unattended Access)

For managed services and contract accounts that require proactive monitoring and background support:
1. **Installation**: The Vestra Support client is installed as a system service during initial onboarding.
2. **Registration**: The client registers itself persistently with the Vestra rendezvous server and associates its unique ID with the client's account records in the Vestra Core API.
3. **Unattended Access**: Under strict access controls, Vestra technicians can launch remote sessions to the managed server or workstation without requiring active user confirmation, utilizing pre-configured encryption keys.
4. **Auditability**: Every connection attempt and session duration is audited and logged back to the Core database for reporting and accountability.

---

## Tactical RMM Integration Concept

Vestra Support is designed with an integration path for [Tactical RMM](https://github.com/amidaware/tacticalrmm) (an open-source Remote Monitoring and Management tool):
* **Direct Launcher**: Integrating a "Vestra Remote" action button directly into the Tactical RMM workstation details view.
* **Command Line Launching**: Passing the client's unique hardware ID and target relay configuration via command-line arguments to the technician viewer, bypassing manual copy-paste steps.
* **Cross-linking**: Linking the machine ID in the RMM database with the active customer account and support ticket in the Vestra Portal.

---

## Relationship to RustDesk

Vestra Support is a **downstream distribution** of RustDesk:
* **Upstream Sync**: The codebase will regularly pull security patches, performance improvements, and feature updates from the upstream RustDesk repository.
* **Compatibility**: The underlying communication protocol, video/audio encoding, and networking architecture remain fully compatible with standard RustDesk components.
* **Pre-configuration**: The main difference is the client configuration; Vestra Support client binaries are compiled with hardcoded, secure defaults for Vestra's private `hbbs` / `hbbr` server endpoints and public keys, removing manual setup steps for the user.

---

## Non-Goals for Version 1 (v1)

To ensure rapid deployment and project stability, the following features are explicitly out of scope for v1:
* **No Protocol Changes**: No modifications to the underlying networking, handshake, or video-streaming protocols.
* **No Custom Encryption**: Strict reliance on RustDesk's existing security implementation (NaCl/sodium and standard TLS).
* **No Authentication Modifications**: Rely on the default ID/password and API token mechanism built into RustDesk without custom auth server modifications.
* **No Platform Porting**: Focus exclusively on Windows and macOS clients for initial releases (Linux/Android/iOS clients will remain stock or deferred).

---

## Current Project Status

* **Phase**: Phase 0 (Scaffolding & Architecture Planning).
* **Code State**: Repository fork has been successfully established and initialized.
* **Documentation**: Main architectural designs, open-source compliance guidelines, and branding rules have been drafted.
* **Next Step**: Deploying the test relay server infrastructure and validating stock RustDesk client connections against it (Phase 1).
