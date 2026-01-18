---
allowed-tools: Bash, Edit, Read, Glob
argument-hint: [version] (e.g., 0.1.0)
description: Automated release process - version bump, tag, publish binaries
---

# Automated Release Process

Execute the complete release workflow for pai-sho project.

## Pre-flight Checks

Current repository status: !`git status`

Current branch: !`git branch --show-current`

Last few releases: !`git tag --sort=-version:refname | head -5`

## Release Steps

### 1. Pre-Release Confirmation

**Ask the user for the following before starting:**
- Cargo registry token (if not already set in environment)
- Confirm the version number: $ARGUMENTS

### 2. Version Management

- Update version in Cargo.toml to $ARGUMENTS
- Run `cargo check` to update Cargo.lock
- Generate changelog from commits since last stable release:
  ```bash
  # Get the previous tag
  previous_tag=$(git tag --list 'v*' --sort=-version:refname | head -1)
  
  # Generate changelog
  if [ -n "$previous_tag" ]; then
    git log --format="- %s" "${previous_tag}..HEAD" > changes/v$ARGUMENTS.md
  else
    git log --format="- %s" HEAD > changes/v$ARGUMENTS.md
  fi
  ```

### 3. Review Release Notes

**⚠️ REVIEW REQUIRED**: The release notes have been generated in
`changes/v$ARGUMENTS.md`. Please review them carefully:

- Check that all important changes are highlighted appropriately
- Edit the highlights section to focus on user-facing improvements
- Ensure the changelog is accurate and complete

**Do not proceed to the next step until you are satisfied with the release notes.**

### 4. Git Operations

- Commit changes with message: `chore: release v$ARGUMENTS`
- Create and push git tag `v$ARGUMENTS`:
  ```bash
  git tag v$ARGUMENTS
  git push origin main
  git push origin v$ARGUMENTS
  ```
- This triggers GitHub workflow to build cross-platform binaries

### 5. Watch CI Workflow

- Get the latest workflow run ID: `gh run list --limit 1`
- Monitor build with: `gh run watch <run-id> --exit-status`
- Wait for all three builds to complete (linux-amd64, linux-arm64, macos-arm64)
- Output the watch command for the user:
  ```
  gh run watch <run-id> --exit-status
  ```

### 6. Verify Release Artifacts

- Verify GitHub release: `gh release view v$ARGUMENTS`
- Ensure all artifacts are uploaded:
  - pai-sho-v$ARGUMENTS-linux-amd64.tar.gz
  - pai-sho-v$ARGUMENTS-linux-arm64.tar.gz
  - pai-sho-v$ARGUMENTS-macos-arm64.tar.gz
- Verify release notes: `gh release view v$ARGUMENTS --json body`
- If release body is just the commit message, update it:
  ```bash
  gh release edit v$ARGUMENTS --notes-file changes/v$ARGUMENTS.md
  ```

### 7. Test Installation

**Verify eget installation works:**

```bash
# Test that eget can install pai-sho
eget cablehead/pai-sho --tag v$ARGUMENTS
```

If eget fails, check:
- Artifact naming matches eget conventions
- Tarball structure is correct (binary in top-level directory)

### 8. Homebrew Formula Update

- Clone `../homebrew-tap` if not present: `git clone https://github.com/cablehead/homebrew-tap.git`
- Download macOS tarball and calculate SHA256:
  ```bash
  cd /tmp
  curl -sL https://github.com/cablehead/pai-sho/releases/download/v$ARGUMENTS/pai-sho-v$ARGUMENTS-macos-arm64.tar.gz -o pai-sho-v$ARGUMENTS-macos-arm64.tar.gz
  sha256sum pai-sho-v$ARGUMENTS-macos-arm64.tar.gz
  ```
- Update `../homebrew-tap/Formula/pai-sho.rb` with new version, URL, and SHA256 checksum
- Commit and push homebrew formula changes

### 9. Manual Verification Required

**⚠️ CRITICAL: macOS Verification BEFORE Publishing to Crates.io**

After homebrew formula is updated, **PAUSE** and ask a macOS user to test:

```bash
brew uninstall pai-sho  # if previously installed
brew install cablehead/tap/pai-sho
pai-sho --version  # should show $ARGUMENTS
```

**STOP HERE if verification fails.** Publishing to crates.io is irreversible.

### 10. Cargo Registry Publication

**Only proceed after macOS verification passes.**

- Use the cargo token provided in step 1: `export CARGO_REGISTRY_TOKEN="..."`
- Run `cargo publish` to publish to crates.io
- **Warning**: This step cannot be undone - you cannot unpublish from crates.io

## Release Complete

The release is now public! Summary:
- ✅ GitHub release: https://github.com/cablehead/pai-sho/releases/tag/v$ARGUMENTS
- ✅ eget: `eget cablehead/pai-sho`
- ✅ Homebrew: `brew install cablehead/tap/pai-sho`
- ✅ Crates.io: `cargo install pai-sho`

## Rollback Plan

If verification fails **before cargo publish**:

1. Delete the git tag:
   ```bash
   git tag -d v$ARGUMENTS
   git push --delete origin v$ARGUMENTS
   ```
2. Delete the GitHub release:
   ```bash
   gh release delete v$ARGUMENTS --yes
   ```
3. Revert homebrew formula changes
4. Revert version changes in Cargo.toml
5. Investigate and fix issues before retry

**Note**: If cargo publish has already completed, you cannot unpublish from crates.io.
You would need to publish a new patch version with the fix instead.

---

**Ready to execute release for version $ARGUMENTS?**
