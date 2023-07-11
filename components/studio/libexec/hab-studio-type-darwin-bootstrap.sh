#!/bin/sh

# TESTING CHANGES
# Documentation on testing local changes to this lives here:
# https://github.com/habitat-sh/habitat/blob/master/BUILDING.md#testing-changes

# # shellcheck disable=2034
studio_type="bootstrap"
studio_env_command="/usr/bin/env"
studio_enter_environment="STUDIO_ENTER=true"
studio_enter_command="$libexec_path/hab pkg exec core/build-tools-hab-backline bash --rcfile $HAB_STUDIO_ROOT/etc/profile"
studio_build_environment=
studio_build_command="$libexec_path/hab pkg exec core/build-tools-hab-plan-build hab-plan-build --"
studio_run_environment=
studio_run_command="$libexec_path/hab pkg exec core/build-tools-hab-backline bash --rcfile $HAB_STUDIO_ROOT/etc/profile"

run_user="hab"
run_group="$run_user"

finish_setup() {
    src_dir="$($pwd_cmd)"
    $mkdir_cmd -p "$HAB_STUDIO_ROOT"/etc
    $mkdir_cmd -p "$HAB_STUDIO_ROOT"/bin
    $mkdir_cmd -p "${HAB_STUDIO_ROOT}${HAB_ROOT_PATH}"/bin

    $cat_cmd <<EOF > "${HAB_STUDIO_ROOT}${HAB_ROOT_PATH}"/bin/build
#!/bin/sh
exec $libexec_path/hab pkg exec core/build-tools-hab-plan-build hab-plan-build "\$@"
EOF
    $chmod_cmd +x "${HAB_STUDIO_ROOT}${HAB_ROOT_PATH}"/bin/build

    $cat_cmd >"$HAB_STUDIO_ROOT"/etc/profile <<PROFILE
if [[ -n "\${STUDIO_ENTER:-}" ]]; then
  unset STUDIO_ENTER
  source $HAB_STUDIO_ROOT/etc/profile.enter
fi

# Add command line completion
source <(hab cli completers --shell bash)
PROFILE

    $cat_cmd >"$HAB_STUDIO_ROOT"/etc/profile.enter <<PROFILE_ENTER
# Source .studiorc so we can apply user-specific configuration
if [[ -f $src_dir/.studiorc && -z "\${HAB_STUDIO_NOSTUDIORC:-}" ]]; then
  echo "--> Detected and loading /src/.studiorc"
  echo ""
  source $src_dir/.studiorc
fi

PROFILE_ENTER

    # Install the hab backline
    "$system_hab_cmd" pkg install "$HAB_STUDIO_BACKLINE_PKG"

    return 0
}
