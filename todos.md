# TODOs

- [ ] Investigate Japanese Unicode input remaining unconfirmed in Windows VS Code.
  - **Scope:** Deferred from the mobile input branch. Address in a dedicated
    Windows input compatibility branch; keep receiver changes out of this branch.
  - **Reproduction:** Focus a native VS Code text editor with the Japanese IME
    active. Inject `日本語` using Windows `SendInput` Unicode events and save.
    Then inject `を` and `勉強` and save again. The saved file contains only
    `日本語`; explicitly confirming with Enter before saving persists the full
    `日本語を勉強`.
  - **Evidence:** The same failure occurs after disconnecting the mobile session,
    using only native Windows injection and desktop Save. The iPad case delivers
    all six characters correctly. Normal Windows Romaji input with explicit
    candidate confirmation passes.
  - **Unresolved:** The responsible Windows IME/TSF/VS Code component is not yet
    identified. Reproduction in this Windows environment does not establish
    exclusivity to Windows or prove that the issue predates this branch; a
    same-condition baseline-version comparison remains outstanding.
  - **Acceptance:** Locally committed text must be saved completely without an
    extra remote confirmation. Automatic Enter or implicit clipboard replacement
    must not be treated as a fix.
