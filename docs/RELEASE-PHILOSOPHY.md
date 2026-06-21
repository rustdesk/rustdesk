# Release Philosophy

This document defines the core principles governing the development, deployment, and operation of Vestra Support.

---

## Core Principles

### 1. Reliability > Features
Remote support tools require absolute, predictable stability. A technician connecting to a client's server in an emergency must be certain the client will connect, authenticate, and display properly:
* We prioritize fixing connection errors, memory leaks, and compilation warnings over implementing new feature hooks or client controls.
* Every release must go through a rigid verification process matching our operational checklist.

### 2. Simplicity > Automation
Avoid premature engineering or unnecessary automation complexity:
* In the early stages of infrastructure setup and build pipelines, we prefer simple, clear, manually-triggerable steps (such as documented docker-compose commands and manual signing steps) over complex, magic scripts.
* We only introduce automation (such as GitHub actions or automated testing loops) once the manual process has been validated, documented, and run consistently.

### 3. Transparency > Magic
All operational configurations, security architectures, and log routes must be transparent and discoverable:
* Pre-configured values (such as server endpoints and public keys) must be clearly documented in our files rather than hidden in compiled binaries or obfuscated variables.
* Remote session logs, connection status notices, and data policies must be clear to both the customer and the operating technician.

### 4. Upstream Compatibility > Custom Forking
Vestra Support is a downstream distribution of RustDesk, not a divergent product:
* To maintain rapid access to security updates, bug fixes, and feature improvements from the open-source community, we must keep our codebase closely aligned with upstream RustDesk.
* We enforce a strict policy of avoiding customization of core networking protocols, encryption loops, or GUI layout code. The custom branding must be implemented as a thin configuration overlay.
