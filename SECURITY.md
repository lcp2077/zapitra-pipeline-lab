# Security policy

ZAPITRA is a design-review and reference-model tool. It does not implement the BLAKE3 Pipeline runtime and does not validate the upstream `unsafe fn lent` implementation.

Please report a vulnerability in ZAPITRA through a private GitHub Security Advisory for this repository. For an upstream BLAKE3 issue, follow the maintainers' security reporting policy instead.

The Rust crate forbids unsafe code. The build and verification scripts do not alter firewall, routing, Wi-Fi, or system security configuration.
