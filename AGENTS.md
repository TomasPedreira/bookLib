# Release workflow

- Batch changes; do not publish an APK after every edit. Publish when the user asks for a release, APK, or version bump.
- Every published APK must have a new version. Never overwrite or replace an APK asset in an existing release.
- For a patch release, increment the patch version unless the user specifies a different version.
- Run the relevant checks before publishing and preserve the Android signing key so updates keep the user's data.
