# Releasing a new version

You release by pushing a version tag. GitHub Actions tests the tagged commit,
builds everything and creates a **draft** release. Nobody without write access
to the repository can see the draft until you publish it.

The examples release version `0.2.0`. Replace it with your version everywhere.

## Steps

### 1. Choose the version number

Versions have the form `MAJOR.MINOR.PATCH`. Before 1.0:

- Bump `PATCH` for bug fixes only: `0.2.0` to `0.2.1`.
- Bump `MINOR` for new features or changed behaviour: `0.2.1` to `0.3.0`.

### 2. Start from an up-to-date `main`

```sh
git switch main
git pull
git status       # should report a clean working tree
```

### 3. Change the version number

Edit `version` under `[package]` in `Cargo.toml`:

```toml
version = "0.2.0"
```

Then update `Cargo.lock`, which holds a copy of the number:

```sh
cargo check
git diff --stat  # should list Cargo.toml and Cargo.lock
```

Don't skip this. The release builds use `--locked`, so they fail if `Cargo.lock`
is out of date.

### 4. Commit and push

```sh
git commit -am "Release 0.2.0"
git push origin main
```

### 5. Tag the commit and push the tag

```sh
git tag v0.2.0
git push origin v0.2.0
```

The tag must be `v` followed by exactly the number in `Cargo.toml`. Pushing it
is what starts the release.

### 6. Wait for the Release workflow

Follow it in the **Actions** tab of the repository. It runs these jobs in order,
and a failure stops the jobs after it:

1. Compare the tag with the version in `Cargo.toml`.
2. Run the full CI workflow (formatting, clippy, tests) on Linux and macOS.
3. Build for every platform.
4. Create the draft release and upload the files.

### 7. Check the draft and publish it

Open the **Releases** page of the repository. The draft should have these files:

| File | For |
|---|---|
| `chronoscope-0.2.0-1.x86_64.rpm` | Fedora, RHEL and similar |
| `chronoscope_0.2.0-1_amd64.deb` | Debian, Ubuntu and similar |
| `chronoscope-v0.2.0-x86_64-unknown-linux-gnu.tar.gz` | Other Linux |
| `chronoscope-v0.2.0-aarch64-apple-darwin.tar.gz` | macOS, Apple silicon |
| `chronoscope-v0.2.0-x86_64-apple-darwin.tar.gz` | macOS, Intel |

Download a package and try it:

```sh
sudo dnf install ./chronoscope-0.2.0-1.x86_64.rpm
chronoscope --version
```

Then edit the generated release notes if you want to, and press **Publish
release**.

## If something goes wrong

**A job failed for a passing reason, such as a network error.** Open the run in
the Actions tab and choose **Re-run failed jobs**. You don't need a new tag.

**The tag is wrong, or a job found a real problem, and the release is not
published yet.** Remove the tag, and the draft if there is one, and start again
with the same version number:

```sh
git tag -d v0.2.0
git push origin --delete v0.2.0
```

To delete a draft, open it on the Releases page and choose **Delete**. Then fix
the problem on `main` and repeat from step 3 (the number may already be right,
in which case go on to step 4 with your fix).

**The release is already published.** Don't move or reuse the tag, because people
may have downloaded those files. Fix the problem and release a new `PATCH`
version.

## Trying a release first

A release candidate runs the whole process without using up the real version
number. Follow the same steps with these names:

| | Release candidate | Real release |
|---|---|---|
| `Cargo.toml` | `0.2.0-rc.1` | `0.2.0` |
| Tag | `v0.2.0-rc.1` | `v0.2.0` |
| Package version | `0.2.0~rc.1` | `0.2.0` |

Package managers treat `0.2.0~rc.1` as older than `0.2.0`, so the real release
upgrades over it. If you publish a candidate, GitHub marks it as a prerelease.
If you only wanted the test, delete the draft and the tag instead.

When you're happy, set the real version in `Cargo.toml` and follow the steps
again. If the candidate needs fixing, use `0.2.0-rc.2` next.

## Test builds between releases

For every commit on `main` that passes CI, the Bundle workflow builds the `.deb`
and `.rpm` and keeps them as artifacts. You'll find them on that workflow's run
in the Actions tab, inside a zip file. `scripts/package.sh` builds the same
packages locally.

These builds all carry the version from `Cargo.toml`, as does a local build. If
that version is already installed, `dnf install` reports `Nothing to do` and
keeps the old build, so replace it with:

```sh
sudo dnf reinstall ./chronoscope-0.2.0-1.x86_64.rpm
```
