# Tactica AUR Package (`tactica-bin`)

`tactica-bin` downloads the pre-compiled Linux binary directly from Tactica's GitHub Releases.

## Automated CI/CD Setup with GitHub Actions

The repository includes [`.github/workflows/release.yml`](../../.github/workflows/release.yml),
which automatically builds the release binary, publishes a rolling GitHub
Release, verifies the published asset, and updates `tactica-bin` on the AUR
whenever `main` is updated. Rolling versions use the format
`r<commit-count>.<short-hash>`.

The package intentionally omits an epoch so the displayed package version is
only `r<commit-count>.<short-hash>-<pkgrel>`.

Users who installed an earlier `epoch=1` build need a one-time explicit
reinstall or downgrade. Subsequent VCS revisions compare normally.

The separate `Build main` workflow only tests and uploads a workflow artifact;
it does not publish to GitHub Releases or the AUR.

### One-Time Setup for GitHub Secrets

1. Copy the public SSH key `~/.ssh/aur_key.pub` and add it to your [AUR Account Settings](https://aur.archlinux.org/account/).
2. Base64-encode the private key as one line:

   ```bash
   base64 -w0 ~/.ssh/aur_key
   ```

3. Add that one-line value as a repository secret named
   `AUR_SSH_PRIVATE_KEY` on GitHub:
   `https://github.com/vcoscrato/Tactica/settings/secrets/actions`

The workflow also accepts a correctly preserved multiline key or a key stored
with literal `\n` separators, but one-line base64 avoids newline corruption.
The release workflow fails explicitly if the secret is missing or invalid, so
a green release run means that the AUR update was pushed successfully.

### Triggering a Release

```bash
git push origin main
```

GitHub Actions builds the binary, creates the GitHub Release, renders
`PKGBUILD.template`, generates and validates `.SRCINFO`, and pushes both files
directly to the AUR.
