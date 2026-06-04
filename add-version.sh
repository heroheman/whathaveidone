#!/bin/bash

# ============================================================================
# add_version.sh - Version Management Script
# ============================================================================
#
# This script automates version bumping and release tagging for this Rust project.
# It reads/writes versions from Cargo.toml, creates git tags, and updates the changelog.
#
# Prerequisites:
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
#   1. Increments version in Cargo.toml
#   2. Asks for confirmation
#   3. Commits the version change
#   4. Creates a git tag (e.g., v0.2.2)
#   5. Updates CHANGELOG.md using git cliff
#   6. Commits the changelog
#   7. Pushes the tag to origin
#
# ============================================================================

# Path to Cargo package manifest
path_to_manifest="Cargo.toml"

# Check if Cargo.toml exists
if [ ! -f "$path_to_manifest" ]; then
    echo "Error: $path_to_manifest not found"
    exit 1
fi

# Extract version from [package] section in Cargo.toml
current_version=$(awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { if (in_package) exit }
    in_package && $1 == "version" {
        gsub(/"/, "", $3)
        print $3
        exit
    }
' "$path_to_manifest")

if [ -z "$current_version" ]; then
    echo "Error: Could not read version from $path_to_manifest"
    exit 1
fi

current_version_without_build=$(echo "$current_version" | sed 's/\+.*//')

set_manifest_version() {
    local version="$1"
    awk -v version="$version" '
        BEGIN { in_package = 0; updated = 0 }
        /^\[package\]$/ { in_package = 1; print; next }
        /^\[/ { in_package = 0 }
        in_package && $1 == "version" && updated == 0 {
            print "version = \"" version "\""
            updated = 1
            next
        }
        { print }
        END {
            if (updated == 0) {
                exit 1
            }
        }
    ' "$path_to_manifest" > "tmp.$$.toml" && mv "tmp.$$.toml" "$path_to_manifest"
}

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
echo "Setting Cargo.toml version $current_version to $new_version"

# Update version in Cargo.toml
if ! set_manifest_version "$new_version"; then
    echo "Error: Failed to write version to $path_to_manifest"
    rm -f "tmp.$$.toml"
    exit 1
fi

# Ask user for confirmation
echo ""
echo "❓ Is version $new_base_version correct? (y/n)"
read -r confirmation

if [[ "$confirmation" != "y" && "$confirmation" != "Y" ]]; then
    echo "Aborted - Reverting changes..."
    # Revert Cargo.toml changes
    set_manifest_version "$current_version"
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

# Stage and commit Cargo.toml changes
echo "Staging and committing Cargo.toml..."
git add "$path_to_manifest"
git commit -am "chore: bump version to $new_base_version"

# Push tag to origin
echo "Pushing tag $TAG to origin..."
git push origin $TAG

echo "✅ Version $new_base_version released and tagged successfully!"
