# E2E tooling for holzi that nixpkgs does not provide as a ready package. Delivered to the
# consumer repo by `com.github.haexmas.atoms.holzi` and read by `nix-devshell-base`'s
# `flake.nix`: a function from `pkgs` to a list of derivations for the devShell.
pkgs:
let
  # The WebDriver bridge Tauri ships for end-to-end tests; it is not in nixpkgs. Built from the
  # crates.io release, so a version bump changes `version` and both hashes together.
  tauri-driver = pkgs.rustPlatform.buildRustPackage rec {
    pname = "tauri-driver";
    version = "2.0.6";
    src = pkgs.fetchCrate {
      inherit pname version;
      hash = "sha256-fTCkEs4NLBW0khaHL4jpVNkrbQg22YPsRMjfJNqnCWA=";
    };
    cargoHash = "sha256-MThAcU+U8PyBGauh3dy7ZRvRX9INmOEeghIlQEGLAPs=";
    # Its tests drive a real browser driver.
    doCheck = false;
  };

  # Only the driver binary of webkitgtk, never the library. The app links the host's
  # webkit2gtk-4.1 (see the holzi molecule's README), and putting `webkitgtk_4_1` itself into the
  # devShell would put its `lib/` on LD_LIBRARY_PATH and its `.pc` files on PKG_CONFIG_PATH and
  # push the host's WebKit aside. The driver speaks WebKit's automation protocol to the app, so it
  # has to be the same version as the host's WebKit: the `version` file lets a runner check that.
  webkit-webdriver = pkgs.runCommand "webkit-webdriver-${pkgs.webkitgtk_4_1.version}" { } ''
    mkdir -p $out/bin $out/share/webkit-webdriver
    ln -s ${pkgs.webkitgtk_4_1}/bin/WebKitWebDriver $out/bin/WebKitWebDriver
    echo ${pkgs.webkitgtk_4_1.version} > $out/share/webkit-webdriver/version
  '';
in
# WebKitGTK and Xvfb are Linux-only; the other platforms have their own drivers.
pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ tauri-driver webkit-webdriver ]
