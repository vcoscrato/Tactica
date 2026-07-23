# Tactica AUR Package (`tactica-bin`)

`tactica-bin` downloads the pre-compiled Linux binary directly from Tactica's GitHub Releases.

## Automated CI/CD Setup with GitHub Actions

The repository includes [`.github/workflows/release.yml`](../../.github/workflows/release.yml),
which automatically builds the release binary, publishes a rolling GitHub
Release, verifies the published asset, and updates `tactica-bin` on the AUR
whenever `main` is updated. Rolling versions use the format
`r<commit-count>.<short-hash>`.

The package uses `epoch=1` because the repository history previously reset the
rolling version from `r6` to `r1`. The epoch ensures that existing `r6`
installations still upgrade to the current release.

The separate `Build main` workflow only tests and uploads a workflow artifact;
it does not publish to GitHub Releases or the AUR.

### One-Time Setup for GitHub Secrets

1. Copy the public SSH key `~/.ssh/aur_key.pub` and add it to your [AUR Account Settings](https://aur.archlinux.org/account/).
2. Copy the private SSH key `~/.ssh/aur_key` and add it as a repository secret named `AUR_SSH_PRIVATE_KEY` on GitHub:
   `https://github.com/vcoscrato/Tactica/settings/secrets/actions`

The release workflow fails explicitly if this secret is missing, so a green
release run means that the AUR update was pushed successfully.

### Triggering a Release

```bash
git push origin main
```

GitHub Actions builds the binary, creates the GitHub Release, renders
`PKGBUILD.template`, generates and validates `.SRCINFO`, and pushes both files
directly to the AUR.
