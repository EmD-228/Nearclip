# Contributing to NearClip

Thanks for helping. NearClip is maintained by one person, so the process is short.

## How changes get in

1. Open an issue first for anything bigger than a small fix, so the approach is agreed before you spend time on it.
2. Fork the repository and create a branch from `main`.
3. Make your change and run the checks listed in the README under "Tests and checks"; CI runs the same ones on every pull request.
4. Open a pull request against `main` describing what changes and why, with a link to the issue. The maintainer reviews, may ask for changes, and merges with a squash commit.

Keep pull requests focused: one change per PR, no unrelated formatting or dependency bumps.

## Conventions

- UI text is in English. Icons come from `@lucide/svelte`; no emoji glyphs in the interface.
- Svelte 5 code uses runes, not Svelte 4 stores.
- The wire protocol, the mDNS service type, the QR scheme and the HKDF labels are compatibility surfaces. Changing any of them needs a protocol version bump and a migration, not a rename.
- Secrets never go in the repository (see README, "Building installers").

## Reporting a security problem

Do not open a public issue. See [SECURITY.md](SECURITY.md).
