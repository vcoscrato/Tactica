# Tactica AUR Package (`tactica-bin`)

`tactica-bin` downloads the pre-compiled Linux binary directly from Tactica's GitHub Releases.

## Automated CI/CD Setup with GitHub Actions

The repository includes [`.github/workflows/release.yml`](../../.github/workflows/release.yml) which automatically builds the release binary, publishes a GitHub Release, and updates `tactica-bin` on the AUR whenever a new version tag (e.g. `v0.4.0`) is pushed to GitHub.

### One-Time Setup for GitHub Secrets:
1. Copy the public SSH key `~/.ssh/aur_key.pub` and add it to your [AUR Account Settings](https://aur.archlinux.org/account/).
2. Copy the private SSH key `~/.ssh/aur_key` and add it as a repository secret named `AUR_SSH_PRIVATE_KEY` on GitHub:
   `https://github.com/vcoscrato/Tactica/settings/secrets/actions`

### Triggering a Release:
```bash
git tag v0.4.0
git push origin v0.4.0
```
GitHub Actions will take care of building the binary, creating the GitHub Release, generating the `.SRCINFO`, updating `PKGBUILD`, and pushing the update directly to the AUR!
