# Setting up the environment

## Officially supported environment: Ubuntu Latest (18.04/Bionic)

([Other environments](#Unsupported-environments) work, but are not officially supported)

You can run this environment natively or in VM. [ISOs for both server and desktop versions are available here](http://releases.ubuntu.com/18.04/).
This installation method uses as many packages from Ubuntu as possible.
If you don't have it, you'll first need to install `git`:
```
sudo -E apt-get install -y --no-install-recommends git
```

Then, clone the codebase and enter the directory:

```
git clone https://github.com/habitat-sh/habitat.git
cd habitat
```

Then, run the system preparation scripts

```
sh support/linux/install_dev_0_ubuntu_latest.sh
sh support/linux/install_dev_9_linux.sh
```

If you want to run the BATS based integration tests you also need docker installed:

``` sh
sh support/linux/install_dev_8_docker.sh
```

Then, make sure rust's `cargo` command is working. You'll need to add `$HOME/.cargo/bin` to your `$PATH`.
On shells that use `.profile`, you can run
```
source ~/.profile
```
For other shells see the documentation for modifying the executable path. For `fish`, you can run
```
set -U fish_user_paths $fish_user_paths "$HOME/.cargo/bin"
```
Check that `cargo` is correctly installed by running
```
cargo --version
```

Next, use our installation script to install rustfmt
```
./support/rustfmt_nightly.sh
```

At any time, you can find the version of rustfmt we are using by running this command at the root level
of the Habitat repo:

```
echo $(< RUSTFMT_VERSION)
```

Then you can run that version of rustfmt on any cargo project.

For example, if:

```
echo $(< RUSTFMT_VERSION)
```

returns "nightly-2019-05-10"

You would run:

```
cargo +nightly-2019-05-10 fmt
```

You may also be able to configure your editor to automatically run rustfmt every time you save. The [./support/rustfmt_nightly.sh](./support/rustfmt_nightly.sh) script may be helpful.

# Compiling habitat binaries

In the root of the `habitat` repo:
```
make
```

The binaries will be in `habitat/target/debug` and can be run directly, or you can use `cargo` commands in the various `components` subdirectories. For example:
```
cd components/sup
cargo run -- --help
cargo run -- status
```

Or from the habitat directory:

```
cargo run -p hab plan --help
cargo run -p hab sup --help
```

## Compiling with symbols for unsupported targets

The [`habitat_core`](components/core/Cargo.toml) crate defines a [feature](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section) for each potential target. `habitat_core` also defines a `supported_targets` feature that enables all supported targets. The supported targets are `x86_64-darwin`, `x86_64-linux`, `aarch64-darwin`, `aarch64-linux`, and `x86_64-windows`. All other targets are unsupported. Their target identifiers exist solely for experimentation.

The `supported_targets` feature is enabled by default. No extra configuration is needed to produce a build aware of these targets. If you would like to produce a build that is aware of an unsupported target, you must set the desired feature flag in all crates that wrap the corresponding `habitat_core` feature flag. Currently, [`hab`](components/hab/Cargo.toml) is the only example of such a crate. For example, to enable the `aarch64-linux` target, follow these steps:

1. Verify that [`habitat_core`](components/core/Cargo.toml) has a feature that corresponds to the `aarch64-linux` target.
2. Determine all crates that use this feature flag. Search all crate's `Cargo.toml` for `habitat_core/aarch64-linux`. We find that the only crate is [`hab`](components/hab/Cargo.toml). The `hab` feature is named the same, `aarch64-linux`, as the `habitat_core` feature.
3. Enable the `aarch64-linux` feature in the `hab` crate. Currently, workspaces can not use the `--features` flag. See [here](https://github.com/rust-lang/cargo/issues/5015). This prevents us from using `cargo build --features "hab/aarch64-linux"` in the workspace folder. This leaves manually editing the [`Cargo.toml`](components/hab/Cargo.toml) file as the best method to enable a feature. This can be accomplished by temporarily adding `aarch64-linux` to the `hab` crate's default feature list. Resulting in the line `default = ["supported_targets", "aarach64-linux"]`.
4. Create a new build.

### Adding a new unsupported target

To add a new unsupported target, it is easiest to follow the changes of another unsupported target (e.g. `aarch64-linux`). The steps are roughly:

1. Add the necessary information to the `package_targets` macro invocation in [components/core/src/package/target.rs](components/core/src/package/target.rs).
2. Add a new [feature](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section) to the [`habitat_core`](components/core/Cargo.toml) crate with the same name as the target.
3. Determine all crates that need to use the new feature as a configuration predicate (e.g. `#[cfg(feature = <new_target>)]`). Add a feature in that crate that wraps the `habitat_core/<new_target>` feature.
4. Add conditional logic using the configuration predicate.

**Note**: Step 3 is required because you can not check if a crate's dependency has a feature set. See [here](https://stackoverflow.com/questions/57792943/can-you-test-if-the-feature-of-a-dependency-is-set-using-the-cfg-macro).

## Compiling launcher

The `hab-launch` binary has a separate build and release process from the rest of the habitat ecosystem.
Generally the launcher does not need to change and it is not necessary to update it with each habitat
release. For details on building and releasing the launcher see
[its README](components/launcher/README.md).

# Testing changes

The `hab` command execs various other binaries such as `hab-sup`. By default, this will run the latest installed habitat package version of the binary. To use your development version, this behavior can be overridden with the following environment variables:
* HAB_BUTTERFLY_BINARY
* HAB_LAUNCH_BINARY
* HAB_STUDIO_BINARY
* HAB_SUP_BINARY

For example, to run your newly-compiled version of `hab-sup` set `HAB_SUP_BINARY` to `/path/to/habitat/target/debug/hab-sup`. This can be done either through exporting (syntax differs, check your shell documentation):
```
export HAB_SUP_BINARY=/path/to/habitat/target/debug/hab-sup
```
or setting the value upon execution of the `hab` binary:
```
env HAB_SUP_BINARY=/path/to/habitat/target/debug/hab-sup hab sup status
```

## Running Unit Tests

In order to exercise the project's unit tests you can either leverage one of the existing platform specific CI scripts or use `cargo test` on the CLI.

Linux CI script
```
$ .expeditor/scripts/verify/run_cargo_test.sh
```

Windows CI script
```
$ .expeditor/scripts/verify/run_cargo_test.ps1
```

Using cargo
```
$ cargo +$(<RUST_NIGHTLY_VERSION) test
```

## Always test the habitat package

These binary overrides can be great for rapid iteration, but will hide errors like [4832](https://github.com/habitat-sh/habitat/issues/4832) and [4834](https://github.com/habitat-sh/habitat/issues/4834). To do a more authentic test:

1. Build a new habitat package
2. Upload to acceptance (or prod, but only in the unstable channel)
3. Install new package and do normal testing

## Testing exporters

Changes to the exporters can be tested once the exporter package has been built locally. For example, to test changes to the container exporter (`chef/hab-pkg-export-container`), first enter the studio and build a new package:

```shell
➤ hab studio enter
…
[1][default:/src:0]# build components/pkg-export-container
…
   hab-pkg-export-container: Installed Path: /hab/pkgs/jasonheath/hab-pkg-export-container/2.0.17/20250110145427
…
```

Now your modifications are installed locally, under `<HAB_ORIGIN>/hab-pkg-export-container`. You can run your new exporter with

```shell
[6][default:/src:0]# hab pkg exec $HAB_ORIGIN/hab-pkg-export-container hab-pkg-export-container --help
hab-pkg-exec 1.6.1243/20241227194506
…
```

Note that the version is updated, confirming you're running the new code. The old version is still accessible by running

``` shell
[10][default:/src:1]# hab pkg export container --help
hab-pkg-export-container 1.6.1243/20241227202254
…
```

## HAB_STUDIO_BINARY

This one is a bit special. Technically [hab-studio.sh](https://github.com/habitat-sh/habitat/blob/master/components/studio/bin/hab-studio.sh) is a shell script file and not binary. This also means that there is no need to build anything; set `HAB_STUDIO_BINARY` to the path to a version of `hab-studio.sh` within a `habitat` checkout and it will be used. This override will also affect which versions of the files in [studio/libexec](https://github.com/habitat-sh/habitat/tree/master/components/studio/libexec) are used. So if you want to test out changes to [hab-studio-profile.sh](https://github.com/habitat-sh/habitat/blob/master/components/studio/libexec/hab-studio-profile.sh) or [hab-studio-type-default.sh](https://github.com/habitat-sh/habitat/blob/master/components/studio/libexec/hab-studio-type-default.sh), make those changes in a checkout of the `habitat` repo located at `/path/to/habitat/repo` and set `HAB_STUDIO_BINARY` to `/path/to/habitat/repo/components/studio/bin/hab-studio.sh`. For example:
```bash
$ env HAB_STUDIO_BINARY=/path/to/habitat/repo/components/studio/bin/hab-studio.sh hab studio enter
```
Once inside the studio, the prompt will indicate that the override is in effect:
```bash
[1][HAB_STUDIO_BINARY][default:/src:0]#
```

# Unsupported environments

## Mac OS X for Linux Development

These install instructions assume you want to develop, build, and run the
various Habitat software components in a Linux environment. The Habitat core
team suggests that you use our consistent development environment that we call
the "devshell" as the easiest way to get started.

1. [Install Docker for Mac](https://www.docker.com/products/docker)
1. Checkout the source by running `git clone git@github.com:habitat-sh/habitat.git; cd habitat`
1. Run `make` to compile all Rust software components (this will take a while)
1. (Optional) Run `make test` if you want to run the tests. This will take a while.

Everything should come up green. Congratulations - you have a working Habitat
development environment.

You can enter a devshell by running `make shell`. This will drop you in a
Docker container at a Bash shell. The source code is mounted in under `/src`,
meaning you can use common Rust workflows such as `cd components/sup; cargo
build`.

**Note:** The Makefile targets are documented. Run `make help` to show the
output (this target requires `perl`).

**Optional:** This project compiles and runs inside Docker containers so while
installing the Rust language isn't strictly necessary, you might want a local
copy of Rust on your workstation (some editors' language support require an
installed version). To [install stable
Rust](https://www.rust-lang.org/install.html), run: `curl -sSf
https://sh.rustup.rs | sh`. Additionally, the project maintainers use the
nightly version of [rustfmt](https://github.com/rust-lang-nursery/rustfmt)
for code formatting. If you are submitting changes, please ensure that your
work has been run through a nightly version of rustfmt. The recommended way to
install it (assuming you have Rust installed as above), is to run
`rustup component add --toolchain nightly rustfmt` and adding `$HOME/.cargo/bin`
to your `PATH`. Actually running it would look something like `cargo +nightly fmt --all`
from the root of the project. The actual toolchain name can vary, depending on which
version of nightly rust you have installed.

**Note2:** While this Docker container will work well for getting started with Habitat development, [you may want to consider using a VM](#vm-vs-docker-development) as you start compiling Habitat components.  To do this, create a VM with your preferred flavor of Linux and follow the appropriate instructions for that flavor below.


## Mac OS X for Native Development

These instructions assume you want to develop, build, and run the various
Habitat software components in a macOS environment. Components other than
the `hab` CLI itself aren't all supported on Mac, but it is sometimes useful
to run parts of the Habitat ecosystem without virtualization. We recommend
using another environment.

First clone the codebase and enter the directory:

```
git clone https://github.com/habitat-sh/habitat.git
cd habitat
```

Then, run the system preparation scripts and try to compile the project:

```
cp components/hab/install.sh /tmp/
sh support/mac/install_dev_0_mac_latest.sh
sh support/mac/install_dev_9_mac.sh
. ~/.profile
export PKG_CONFIG_PATH="/usr/local/opt/openssl/lib/pkgconfig:/usr/local/lib/pkgconfig:$PKG_CONFIG_PATH"
export IN_DOCKER=false
make
```

For the builds to find the openssl libraries, you will need to
set the `PKG_CONFIG_PATH` environment variable as above before running `cargo
build`, `cargo test`, etc. Additionally to use the Makefile on Mac and not have
the tasks execute in the Docker-based devshell (see above), you will need to
set `IN_DOCKER=false` in your environment. If you use an environment switcher
such as [direnv](https://direnv.net/), you can set up the following in the root
of the git repository:

```
echo 'export PKG_CONFIG_PATH="/usr/local/opt/openssl/lib/pkgconfig:/usr/local/lib/pkgconfig:$PKG_CONFIG_PATH"' > .envrc
direnv allow
```

## Ubuntu: 14.04+ (Trusty+)

This can be used to build and install on older versions of Ubuntu where
libsodium and czmq aren't available.

First clone the codebase and enter the directory:

```
git clone https://github.com/habitat-sh/habitat.git
cd habitat
```

Then, run the system preparation scripts and try to compile the project:

```
sh support/linux/install_dev_0_ubuntu_14.04.sh
sh support/linux/install_dev_9_linux.sh
. ~/.profile
make
```

These docs were tested with a Docker image, created as follows:

```
docker run --rm -it ubuntu:trusty bash
apt-get update && apt-get install -y sudo git-core
useradd -m -s /bin/bash -G sudo jdoe
echo jdoe:1234 | chpasswd
sudo su - jdoe
```


## Centos 7

First clone the codebase and enter the directory:

```
git clone https://github.com/habitat-sh/habitat.git
cd habitat
```

Then, run the system preparation scripts and try to compile the project:

```
sh support/linux/install_dev_0_centos_7.sh
sh support/linux/install_dev_9_linux.sh
. ~/.profile
make
```

If you have issues with libsodium at runtime, ensure that you've set
`LD_LIBRARY_PATH` and `PKG_CONFIG_PATH`:

    export LD_LIBRARY_PATH=/usr/local/lib
    export PKG_CONFIG_PATH=/usr/local/lib/pkgconfig

These docs were tested with a Docker image, created as follows:

```
docker run --rm -it centos:7 bash
yum install -y sudo git
useradd -m -s /bin/bash -G wheel jdoe
echo jdoe:1234 | chpasswd
sudo su - jdoe
```


## Arch Linux

First clone the codebase and enter the directory:

```
git clone https://github.com/habitat-sh/habitat.git
cd habitat
```

Then, run the system preparation scripts and try to compile the project:

```
sh support/linux/install_dev_0_arch.sh
sh support/linux/install_dev_9_linux.sh
. ~/.profile
make
```

These docs were tested with a Docker image, created as follows:

```
docker run --rm -it greyltc/archlinux bash
pacman -Syy --noconfirm
pacman -S --noconfirm sudo git
echo "%wheel ALL=(ALL) ALL" > /etc/sudoers.d/01_wheel
useradd -m -s /bin/bash -G wheel jdoe
echo jdoe:1234 | chpasswd
sudo su - jdoe
```

## VM vs. Docker development

While the available docker images can provide a convenient contributor onboarding experience, they may not prove ideal for extensive habitat development. Building habitat components is a disk intensive operation. Mounted file systems across network boundaries, even when confined to a single local host, can yield build times dramatically slower than building against a locally attached file system. Build times will be best on either a bare metal linux environment or a dedicated linux vm. Also note that when using vagrant to provision a vm for habitat development, it is advised that you do not put your habitat repo on a synced folder or at least do not use that folder as your build target.

The following directions clone the habitat repository into a local directory to realize faster build times; **however**, while faster, this approach carries the risk of losing your work if your VM should crash or get into an unrecoverable state. If developing in a VM, intermittently pushing a WIP to your Habitat fork can protect your progress.

In the root of the project is a Vagrantfile that provisions an Ubuntu environment for Habitat development:

```
vagrant up --provider virtualbox  # See the Vagrantfile for additional providers and boxes
```

Feel free to use this file as a jumping-off point for customizing your own Habitat development environment.

```
vagrant ssh
sudo su -
git clone https://github.com/habitat-sh/habitat.git
cd habitat/components/builder-api
cargo test
```

## Windows

These instructions are based on Windows 10 1607 (Anniversary update) or newer.
Most of it will probably work downlevel.

All commands are in PowerShell unless otherwise stated.  It is assumed that you
have `git` installed and configured.  Posh-Git is a handy PowerShell module for
making `git` better in your PowerShell console (`install-module posh-git`).

```
# Clone the Habitat source
git clone https://github.com/habitat-sh/habitat.git

cd habitat
./build.ps1 components/hab -configure
```
