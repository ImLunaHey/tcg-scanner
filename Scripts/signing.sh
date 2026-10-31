#!/bin/bash
# Sourced by deployment scripts only. Runtime services never invoke codesign.

scanner_load_signing_identity() {
    local scanner_identity_file="$HOME/Library/Application Support/Scanner/signing-identity"
    scanner_signing_identity="${SCANNER_SIGNING_IDENTITY:-}"
    if [[ -z "$scanner_signing_identity" && -f "$scanner_identity_file" ]]; then
        scanner_signing_identity="$(cat "$scanner_identity_file")"
    fi
    if [[ ! "$scanner_signing_identity" =~ ^[[:xdigit:]]{40}$ ]]; then
        echo 'Configure a certificate SHA-1 in SCANNER_SIGNING_IDENTITY or Application Support/Scanner/signing-identity. See Docs/signing.md. Ad-hoc fallback is disabled.' >&2
        return 1
    fi
    scanner_signing_identity="$(printf '%s' "$scanner_signing_identity" | tr '[:lower:]' '[:upper:]')"
    local scanner_available_identities
    scanner_available_identities="$(security find-identity -v -p codesigning)" || return
    if [[ "$scanner_available_identities" != *"$scanner_signing_identity"* ]]; then
        echo 'Configured signing certificate and private key are unavailable or invalid. Restore them in Keychain; do not switch to ad-hoc signing.' >&2
        return 1
    fi
}

scanner_sign() {
    local scanner_sign_path="$1"
    local scanner_sign_identifier="$2"
    codesign --force --sign "$scanner_signing_identity" --identifier "$scanner_sign_identifier" "$scanner_sign_path" || return
    codesign --verify --strict "$scanner_sign_path"
}
