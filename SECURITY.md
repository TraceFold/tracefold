# Security Policy

Tracefold takes security and cryptographic integrity seriously. As a critical infrastructure component for AI agent tool governance, we follow strict disclosure and verification standards.

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability or cryptographic flaw in Tracefold:
1. Please **do NOT** open a public issue.
2. Report the vulnerability privately via GitHub Security Advisories or by emailing `security@tracefold.dev`.
3. Provide a minimal reproduction script or fuzz test vector.
4. We aim to acknowledge reports within 24 hours and provide a fix within 7 days.

## Cryptographic Commitments

- **Signatures**: Ed25519 DSSE (in-toto v1.0 standard envelope).
- **Hashing**: BLAKE3 for cryptographic preimage and Merkle tile digests.
- **Verification**: Fully offline; zero external network dependencies required during receipt evaluation.
