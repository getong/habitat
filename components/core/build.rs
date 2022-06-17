use std::{env,
          fs,
          path::Path};

#[cfg(windows)]
fn main() {
    use std::{fs::File,
              io::prelude::*};

    cc::Build::new().file("./src/os/users/admincheck.c")
                    .compile("libadmincheck.a");
    let mut file =
        File::create(Path::new(&env::var("OUT_DIR").unwrap()).join("hab-crypt")).unwrap();
    if let Ok(key) = env::var("HAB_CRYPTO_KEY") {
        file.write_all(&base64::decode(&key).unwrap()).unwrap();
    }

    populate_cacert();
}

#[cfg(not(windows))]
fn main() { populate_cacert(); }

pub fn populate_cacert() {
    if let Ok(src) = env::var("SSL_CERT_FILE") {
        let dst = Path::new(&env::var("OUT_DIR").unwrap()).join("cacert.pem");
        if !dst.exists() {
            fs::copy(&src, &dst).unwrap_or_else(|_| {
                                    panic!("Failed to copy CA certificates from '{}' to '{}' for \
                                            compiliation.",
                                           src,
                                           dst.display())
                                });
        }
    } else if env::var("PROFILE").unwrap() == "release" {
        panic!("SSL_CERT_FILE environment variable must contain path to minimal CA certificates \
                files to be used by Habitat in environments where core/cacerts package or native \
                platform CA certificates are not available.");
    } else {
        let dst = Path::new(&env::var("OUT_DIR").unwrap()).join("cacert.pem");
        fs::write(&dst, "").unwrap_or_else(|_| {
                               panic!("Failed to write empty CA certificates file at '{}' for \
                                       compiliation.",
                                      dst.display())
                           });
        println!("cargo:warning=SSL_CERT_FILE environment variable is not specified. Habitat \
                  will be built without a minimal set of CA root certificates. This may cause it \
                  to fail on https requests in environments where core/cacerts package or native \
                  platform CA certificates are not available.");
    }
}
