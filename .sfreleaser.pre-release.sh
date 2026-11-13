#!/usr/bin/env bash

ROOT="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

force=false
repository="github.com/streamingfast/substreams-skills"

main() {
  pushd "$ROOT" &> /dev/null

  while getopts "hf" opt; do
    case $opt in
      h) usage && exit 0;;
      f) force=true;;
      \?) usage_error "Invalid option: -$OPTARG";;
    esac
  done
  shift $((OPTIND-1))

  version="$1"; shift
  if [[ $version == "" ]]; then
    usage_error "parameter <version> is required"
  fi

  version="${version#v}" # Remove leading 'v' if present

  check_git_clean

  # Update package.json version
  perl -pi -e "s/\"version\": \".*?\"/\"version\": \"${version}\"/" package.json

  # Update Claude Code plugin manifest version
  if [[ -f .claude-plugin/plugin.json ]]; then
    perl -pi -e "s/\"version\": \".*?\"/\"version\": \"${version}\"/" .claude-plugin/plugin.json
  fi

  # Update all SKILL.md files in skills/ directory
  for skill_file in skills/*/SKILL.md; do
    if [[ -f "$skill_file" ]]; then
      perl -pi -e "s/^  version: .*/  version: ${version}/" "$skill_file"
    fi
  done

  # Update CHANGELOG.md
  perl -pi -e "s{^## \\[Unreleased\\]}{## [${version}](https://${repository}/releases/tag/v${version})}" CHANGELOG.md

  # Validate the project
  [[ -d node_modules ]] || npm ci --no-audit --no-fund
  npm run validate

  if [[ -n $(git status --porcelain) ]]; then
    git add -A .
    git commit -m "Preparing release of ${version}"
  fi
}

check_git_clean() {
  if [[ "$force" == true ]]; then
    return
  fi

  if [[ -n $(git status --porcelain) ]]; then
    echo "ERROR: Your git working directory is not clean. Please commit or stash your changes before running this script."
    exit 1
  fi
}

usage_error() {
  message="$1"
  exit_code="$2"

  echo "ERROR: $message"
  echo ""
  usage
  exit ${exit_code:-1}
}

usage() {
  echo "usage: .sfreleaser.pre-release.sh <version>"
  echo ""
  echo "Run the necessary pre-release tasks for updating the version of"
  echo "the project in various files:"
  echo ""
  echo "This script will update the following files with the new version:"
  echo "  - package.json: Updates the 'version' field to <version>"
  echo "  - .claude-plugin/plugin.json: Updates the 'version' field to <version>"
  echo "  - skills/*/SKILL.md: Updates the 'version' field in metadata to <version>"
  echo "  - CHANGELOG.md: Replaces '## [Unreleased]' with '## [<version>](<release link>)'"
  echo ""
  echo "It will also run 'npm run validate' to validate the skills."
  echo ""
  echo "Options"
  echo "    -f          Force the script to run even if the git working directory is not clean"
  echo "    -h          Display help about this script"
}

main "$@"
