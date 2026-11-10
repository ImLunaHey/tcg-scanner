# Stable local signing

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

The installed app and services must retain their signing identity across updates
so macOS can associate privacy approvals with the same software. Ad-hoc signatures
identify exact binary contents and change when a build changes. Signing only the
GUI does not fix network-volume prompts from the separate `scanner-service`.
See Apple's [Inside Code Signing: Requirements](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements).

## Configure this Mac

Use an available Apple Development certificate with its private key for local
development. List identities with:

```sh
security find-identity -v -p codesigning
```

Set `SCANNER_SIGNING_IDENTITY` to the selected certificate's 40-character SHA-1,
or store that value on one line in:

```text
~/Library/Application Support/Scanner/signing-identity
```

This local file contains a certificate identifier, not a private key or password.
Keep it outside Git; private keys remain in Keychain. This installation has been
configured to use its existing Apple Development identity. Keychain may ask to
allow `codesign` to use that key.

The shared `Scripts/signing.sh` helper checks that the configured identity is
available and valid before building/installing, signs with fixed identifiers, and
verifies the resulting signature. Missing identities fail explicitly. There is
no silent fallback to ad-hoc signing.

| Component | Signing identifier |
| --- | --- |
| Native app | `local.scanner.mac` |
| Hardware service | `local.scanner.service` |
| CLI | `local.scanner.cli` |
| MCP server | `local.scanner.mcp` |
| HTTP gateway | `local.scanner.http` |

Use the normal build/install scripts for subsequent updates. When using Xcode
directly, select the same development team under Signing & Capabilities; the
project uses Apple Development signing. Builds made with `CODE_SIGNING_ALLOWED=NO`
are compile checks only and must not replace the installed signed application.

## Permissions and verification

Changing from the previous ad-hoc identity can require one final network-volume
approval for `scanner-service`. A certificate-backed designated requirement should
remain stable for future builds using the same identity and identifier. This does
not grant permissions automatically or guarantee macOS will never ask again:
changing signing identities, resetting privacy settings, or changing process
attribution can require renewed approval. Do not edit the privacy database or
disable security controls to suppress a prompt.

Inspect the installed signatures:

```sh
codesign -d -r- "$HOME/Library/Application Support/Scanner/bin/scanner-service"
codesign --verify --strict "$HOME/Library/Application Support/Scanner/bin/scanner-service"
codesign -d -r- "$HOME/Applications/Scanner.app"
codesign --verify --strict "$HOME/Applications/Scanner.app"
```

The designated requirement should identify the certificate and fixed identifier,
not only a `cdhash`. During implementation, distinct debug and release service
binaries were signed, found to have identical designated requirements, and each
verified against that requirement. Ad-hoc (`-`), malformed and unavailable
identity configurations were confirmed to fail.

### Live validation — 2026-10-31

The native app, scanner service, CLI, MCP and HTTP gateway were installed with
certificate-backed signatures and their fixed identifiers. Signature verification
passed for every installed component. HTTP authentication and the existing
Tailscale configuration were preserved.

After the user approved the initial identity change, the signed service retrieved
all 11 originals and two retained edits with matching SHA-256 hashes and generated
their thumbnails. An actual update test then stopped the idle service, installed
the differently built signed debug executable at the same path, and restarted it.
It retrieved an existing SMB original without another permission approval. The
signed release executable was restored and restarted, and the same read again
succeeded without another approval. Both reads verified the original's full hash.
No scan or paper feeding was initiated. The service finished idle on the release
build with `/Volumes/scans` as its storage root.

Private pre-update binaries and deployment configuration are backed up under
`~/Library/Application Support/Scanner/backups/signing-20261031T183838Z/`.
Signing requirement and live-update verification records are retained there too;
build fixtures remain ignored under `.build/signing-validation/`.

This is local development signing. Distribution to other Macs would require a
separate Developer ID signing/notarization workflow; that is not implemented here.
