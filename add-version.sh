#!/bin/bash

# ============================================================================
# add_version.sh - Version Management Script
# ============================================================================
#
# This script automates version bumping and release tagging for this Nuxt project.
# It reads/writes versions from package.json, creates git tags, and updates the changelog.
#
# Prerequisites:
# - jq (install via: brew install jq)
# - git cliff (install via: brew install git-cliff)
#
# Usage:
#   ./add_version.sh                    # Auto-increment patch (0.2.1 → 0.2.2)
#   ./add_version.sh --semver major     # Increment major (0.2.1 → 1.0.0)
#   ./add_version.sh --semver minor     # Increment minor (0.2.1 → 0.3.0)
#   ./add_version.sh --semver patch     # Increment patch (0.2.1 → 0.2.2)
#   ./add_version.sh --version X.Y.Z    # Set specific version (e.g., 1.2.3)
#
# What it does:
#   1. Increments version in package.json (adds +gitcount as build number)
#   2. Asks for confirmation
#   3. Commits the version change
#   4. Creates a git tag (e.g., v0.2.2)
#   5. Updates CHANGELOG.md using git cliff
#   6. Commits the changelog
#   7. Pushes the tag to origin
#
# ============================================================================

path_to_package="package.json"

# Check if jq is installed
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed. Install it with: brew install jq"
    exit 1
fi

current_version=$(jq -r '.version' $path_to_package)
current_version_without_build=$(echo "$current_version" | sed 's/\+.*//')

# Parse current semver version
IFS='.' read -r major minor patch <<< "$current_version_without_build"

# Determine new version based on argument
if [ -z "$1" ]; then
    # No argument: increment patch by 1
    patch=$((patch + 1))
    new_base_version="$major.$minor.$patch"
    echo "No version specified, auto-incrementing patch: $current_version_without_build -> $new_base_version"
elif [ "$1" == "--semver" ]; then
    # Semver increment
    case "$2" in
        major)
            major=$((major + 1))
            minor=0
            patch=0
            new_base_version="$major.$minor.$patch"
            echo "Incrementing major version: $current_version_without_build -> $new_base_version"
            ;;
        minor)
            minor=$((minor + 1))
            patch=0
            new_base_version="$major.$minor.$patch"
            echo "Incrementing minor version: $current_version_without_build -> $new_base_version"
            ;;
        patch)
            patch=$((patch + 1))
            new_base_version="$major.$minor.$patch"
            echo "Incrementing patch version: $current_version_without_build -> $new_base_version"
            ;;
        *)
            echo "Error: Invalid semver argument. Use: major, minor, or patch"
            exit 1
            ;;
    esac
elif [ "$1" == "--version" ]; then
    # Custom version provided
    new_base_version="$2"
    echo "Using provided version: $new_base_version"
else
    echo "Error: Invalid argument. Usage:"
    echo "  ./add_version.sh                    # Auto-increment patch"
    echo "  ./add_version.sh --semver major     # Increment major version"
    echo "  ./add_version.sh --semver minor     # Increment minor version"
    echo "  ./add_version.sh --semver patch     # Increment patch version"
    echo "  ./add_version.sh --version X.Y.Z    # Set specific version"
    exit 1
fi

# Add git count as build number
gitcount=`git log | grep "^commit" | wc -l | xargs`
# new_version="$new_base_version+$gitcount"
new_version="$new_base_version"
echo "Setting package.json version $current_version to $new_version"

# Update version in package.json using jq
jq --arg version "$new_version" '.version = $version' $path_to_package > tmp.$$.json && mv tmp.$$.json $path_to_package

# Ask user for confirmation
echo ""
echo "❓ Is version $new_base_version correct? (y/n)"
read -r confirmation

if [[ "$confirmation" != "y" && "$confirmation" != "Y" ]]; then
    echo "❌ Aborted - Reverting changes..."
    # Revert package.json changes
    jq --arg version "$current_version" '.version = $version' $path_to_package > tmp.$$.json && mv tmp.$$.json $path_to_package
    exit 1
fi

# Create git tag
TAG="v$new_base_version"
echo "Creating git tag $TAG..."
git tag -a $TAG -m "$TAG"

# Update changelog with git cliff
echo "Updating changelog with git cliff..."
git cliff --output CHANGELOG.md

# Commit changelog
# echo "Committing changelog..."
# git commit -am "chore: changelog update"

# Stage and commit package.json changes
echo "Staging and committing package.json..."
git add $path_to_package
git commit -am "chore: bump version to $new_base_version"

# Push tag to origin
echo "Pushing tag $TAG to origin..."
git push origin $TAG

echo "✅ Version $new_base_version released and tagged successfully!"
