#!/bin/bash

source .expeditor/scripts/shared.sh

### This file should include things that are used exclusively by the release pipeline

get_release_channel() {
    echo "habitat-release-${BUILDKITE_BUILD_ID}"
}

# Read the contents of the VERSION file. This will be used to
# determine where generated artifacts go in S3.
#
# As long as you don't `cd` out of the repository, this will do the
# trick.
get_version_from_repo() {
    dir="$(git rev-parse --show-toplevel)"
    cat "$dir/VERSION"
}

# Download public and private keys for the "core" origin from Builder.
#
# Currently relies on a global variable `hab_binary` being set, since
# in the Linux build process, we need to switch binaries mid-way
# through the pipeline. As we bring more platforms into play, this may
# change. FYI.
import_keys() {
    echo "--- :key: Downloading 'chef' public keys from ${HAB_BLDR_URL}"
    ${hab_binary:?} origin key download chef
    echo "--- :closed_lock_with_key: Downloading latest 'chef' secret key from ${HAB_BLDR_URL}"
    ${hab_binary:?} origin key download \
        --auth="${HAB_AUTH_TOKEN}" \
        --secret \
        chef
}

# Returns the full "release" version in the form of X.Y.Z/DATESTAMP
get_latest_pkg_release_version_in_release_channel() {
    local pkg_name="${1}"
    local pkg_target="${2}"
    curl -s "${HAB_BLDR_URL}/v1/depot/channels/chef/$(get_release_channel)/pkgs/${pkg_name}/latest?target=${pkg_target}" \
        | jq -r '.ident | .version + "/" + .release'
}

# Returns the semver version in the form of X.Y.Z
get_latest_pkg_version_in_release_channel() {
    local pkg_name="${1}"
    local pkg_target="${2}"
    local release
    release=$(get_latest_pkg_release_version_in_release_channel "${pkg_name}" "${pkg_target}")
    echo "$release" | cut -f1 -d"/"
}

# Install the latest binary from the release channel and set the `hab_binary` variable
#
# Requires the `hab_binary` global variable is already set!
#
# *Requires* a pkg target argument.
install_release_channel_hab_binary() {
    local pkg_target="${1}"
    curlbash_hab "${pkg_target}"

    echo "--- :habicat: Installed latest acceptance hab: $(${hab_binary} --version)"
    # now install the latest hab available in our channel, if it and the studio exist yet
    hab_version=$(get_latest_pkg_version_in_release_channel "hab" "${pkg_target}")
    studio_version=$(get_latest_pkg_version_in_release_channel "hab-studio" "${pkg_target}")

    if [[ -n $hab_version && -n $studio_version && $hab_version == "$studio_version" ]]; then
        echo "--- :habicat: Hab and studio versions match! Found hab: ${hab_version} - studio: ${studio_version}. Upgrading! :metal:"
        channel=$(get_release_channel)
        "${hab_binary:?}" pkg install --binlink --force --channel "${channel}" chef/hab

        # TODO (CM): Not exactly sure why, but if we call this using
        # `hab` rather than `${hab_binary}`, then we get the wrong
        # version back... related to Workstation's `hab` binary being
        # earlier on the path?
        hab_binary="$(${hab_binary} pkg path chef/hab)/bin/hab"

        "${hab_binary:?}" pkg install --binlink --force --channel "${channel}" chef/hab-studio
        echo "Installed latest build hab: $(${hab_binary} --version) at $hab_binary"
    else
        echo "--- :habicat: Hab and studio versions did not match. hab: ${hab_version:-null} - studio: ${studio_version:-null}"
    fi
}

# Until we can reliably deal with packages that have the same
# identifier, but different target, we'll track the information in
# Buildkite metadata.
#
# Each time we put a package into our release channel, we'll record
# what target it was built for.
set_target_metadata() {
    package_ident="${1}"
    target="${2}"

    echo "--- :partyparrot: Setting target metadata for '${package_ident}' (${target})"
    buildkite-agent meta-data set "${package_ident}-${target}" "true"
}

# When we do the final promotions, we need to know the target of each
# package in order to properly get the promotion done. If Buildkite metadata for
# an ident/target pair exists, then that means that's a valid
# combination, and we can use the target in the promotion call.
ident_has_target() {
    package_ident="${1}"
    target="${2}"

    echo "--- :partyparrot: Checking target metadata for '${package_ident}' (${target})"
    buildkite-agent meta-data exists "${package_ident}-${target}"
}

get_version_from_hart() {
    local hart="${1}"
    # Note that we redirect stderr to /dev/null to compensate for
    # a bug that emits deperecated alias warnings from `hab pkg info`
    # we can stop doing this once the next release after 1.6.235 lands
    # in stable
    ${hab_binary} pkg info --json "${hart}" 2>/dev/null | jq -r '.version'
}

get_release_from_hart() {
    local hart="${1}"
    # Note that we redirect stderr to /dev/null to compensate for
    # a bug that emits deperecated alias warnings from `hab pkg info`
    # we can stop doing this once the next release after 1.6.235 lands
    # in stable
    ${hab_binary} pkg info --json "${hart}" 2>/dev/null | jq -r '.release'
}

# This bit of magic strips off the Habitat header (first 6 lines) from
# the compressed tar file that is a chef/hab .hart, and extracts the
# contents of the `bin` directory only, into the ${archive_dir}
# directory.
#
# For Linux and macOS packages, this will just include the single
# `hab` binary, but on Windows, it will include `hab.exe`, as well as
# all the DLL files needed to run it.
#
# At the end of the day, that's all we need to package up in a
# Habitat-agnostic archive.
#
# Note that `dir` should exist before calling this function.
extract_hab_binaries_from_hart() {
    local hart="${1}"
    local dir="${2}"
    local origin="${3:-chef}"

    tail --lines=+6 "${hart}" | \
        tar --extract \
            --directory="${dir}" \
            --xz \
            --strip-components=7 \
            --wildcards "hab/pkgs/${origin}/hab/*/*/bin/"
}

make_tarball() {
    local name="${1}"
    local dir="${2}"

    local artifact="${name}.tar.gz"
    # Don't use --verbose, since this is called in various contexts
    # where output may not be welcome.
    tar --create \
        "${dir}" | gzip --best > "${artifact}"
}

make_zip() {
    local name="${1}"
    local dir="${2}"

    local artifact="${name}.zip"
    # Similar to the tar command above, keep the noise level down,
    # since this is called in various contexts where output may not be
    # welcome.
    zip --quiet -9 -r "${artifact}" "${dir}"
}

internal_archive_dir_name() {
    local hart="${1}"
    local target="${2}"
    local hab_version
    hab_version="$(get_version_from_hart "${hart}")"
    local hab_release
    hab_release="$(get_release_from_hart "${hart}")"

    echo "hab-${hab_version}-${hab_release}-${target}"
}

create_archive_from_hart() {
    local hart="${1}"
    local target="${2}"
    local origin="${3:-chef}"

    local archive_dir
    archive_dir="$(internal_archive_dir_name "${hart}" "${target}")"
    mkdir "${archive_dir}"

    extract_hab_binaries_from_hart "${hart}" "${archive_dir}" "${origin}"

    pkg_name="hab-${target}"

    case "$target" in
        *-linux)
            pkg_artifact="${pkg_name}.tar.gz"
            make_tarball "${pkg_name}" "${archive_dir}"
            ;;
        *-darwin | *-windows)
            # TODO (CM): Why a zip for macOS?
            pkg_artifact="${pkg_name}.zip"
            make_zip "${pkg_name}" "${archive_dir}"
            ;;
        *)
            echo "${hart} has unknown TARGET=$BUILD_PKG_TARGET"
            exit 3
            ;;
    esac

    echo "${pkg_artifact}"
}
