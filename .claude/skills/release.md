# Release Skill

Create a new release by bumping the version, generating a changelog, and pushing a version tag.

## Steps

### 1. Determine Current Version and Suggest Next Version

- Get the latest version tag from git (filter out `dev-*` and `latest` tags)
- Parse the version number (format: `vMAJOR.MINOR.PATCH`)
- Calculate and suggest three options:
  - **Patch**: Increment patch version (e.g., `v0.1.5` → `v0.1.6`)
  - **Minor**: Increment minor version, reset patch (e.g., `v0.1.5` → `v0.2.0`)
  - **Major**: Increment major version, reset minor and patch (e.g., `v0.1.5` → `v1.0.0`)
- Use `AskUserQuestion` to let the user:
  - Select one of the three suggestions
  - Or choose "Other" to provide a custom version number
- Validate the selected version:
  - Must start with `v`
  - Must follow semver format (`vX.Y.Z`)
  - Must be greater than the current version
  - Must not already exist as a tag

### 2. Generate Changelog Entry

- Get all commits since the last version tag: `git log <last_tag>..HEAD --oneline`
- Parse the commits and categorize them:
  - Commits starting with "Add", "Implement", "Create" → **Added** section
  - Commits starting with "Fix", "Resolve" → **Fixed** section
  - Commits starting with "Update", "Change", "Refactor", "Improve" → **Changed** section
  - Other commits → **Changed** section (default)
- Generate a changelog entry following this format:
  ```markdown
  ## [X.Y.Z] - YYYY-MM-DD

  ### Added
  - Feature 1
  - Feature 2

  ### Changed
  - Change 1
  - Change 2

  ### Fixed
  - Fix 1
  - Fix 2
  ```
- Use today's date in `YYYY-MM-DD` format
- Remove the `v` prefix from version in the heading (e.g., `## [0.1.6]` not `## [v0.1.6]`)
- If a section is empty, omit it entirely

### 3. Confirm or Edit Changelog

- Display the generated changelog entry to the user
- Use `AskUserQuestion` with a multiselect option to let the user:
  - Accept the changelog as-is
  - Edit the changelog (if user wants to edit, ask them to provide the full edited changelog text)
- If user provides edited text, validate:
  - It follows the basic format (starts with `## [version] - date`)
  - It's not empty

### 4. Update CHANGELOG.md and Create Release Notes

- Read the existing `CHANGELOG.md` file
- Insert the new changelog entry after the header (after line 6, before the first version entry)
- Add a blank line between the new entry and the previous first entry
- Write the updated content back to `CHANGELOG.md`
- Show the user a preview of the change

- Create `RELEASE_NOTES.txt` with the changelog entry content (without the version header):
  - Include only the body of the changelog (the Added/Changed/Fixed sections)
  - This file will be read by CI to populate TestFlight "What to Test" notes
  - Format example:
    ```
    ### Added
    - Feature 1
    - Feature 2

    ### Changed
    - Change 1

    ### Fixed
    - Fix 1
    ```

### 5. Commit and Push Changes

- Stage both files: `git add CHANGELOG.md RELEASE_NOTES.txt`
- Commit with message: `Update CHANGELOG for <version>`
- Push to origin: `git push origin main` (or current branch)
- Confirm the commit was pushed successfully

### 6. Create and Push Tag

- Create the git tag: `git tag <version>`
- Push the tag to origin: `git push origin <version>`
- Confirm to the user that:
  - The tag has been created and pushed
  - The tag points to the commit that includes the CHANGELOG update
  - GitHub Actions will automatically build and create releases for:
    - Desktop (macOS, Windows, Linux)
    - macOS App Store
    - iOS TestFlight
    - Android
  - They can monitor the progress at: `https://github.com/{owner}/{repo}/actions`

## Important Notes

- **Always use `AskUserQuestion`** to get user confirmation before:
  - Finalizing the version number
  - Finalizing the changelog text
  - Pushing tags or commits
- **Never guess** the version number or changelog content
- If there are no commits since the last tag, inform the user and ask if they still want to create a release
- If the git working directory is dirty (uncommitted changes), warn the user and ask if they want to continue
- The GitHub Actions workflow (`.github/workflows/build.yml`) will automatically:
  - Build all platform binaries when a `v*` tag is pushed
  - Upload to App Store Connect (macOS and iOS)
  - Create a GitHub release with artifacts

## Error Handling

- If no version tags exist, start from `v0.1.0`
- If git commands fail, show the error to the user and stop
- If the user provides an invalid version number, explain why and ask again
- If pushing the tag fails (e.g., already exists remotely), show the error and suggest solutions
