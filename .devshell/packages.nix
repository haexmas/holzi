# Tooling for holzi that nixpkgs does not provide as a ready package: the e2e drivers and the
# Android SDK with its NDK. Delivered to the consumer repo by `com.github.haexmas.atoms.holzi` and
# read by `nix-devshell-base`'s `flake.nix`: a function from `pkgs` to a list of derivations for the
# devShell.
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

  # The Android SDK for `tauri android init/build`: one platform, the build tools Gradle asks for,
  # the platform tools and one NDK; no emulator and no system images (a device or an emulator of
  # the host is used). Adopting this molecule accepts the Android SDK license, as `sdkmanager
  # --licenses` would.
  ndkVersion = "28.2.13676358";
  android = (pkgs.androidenv.override { licenseAccepted = true; }).composeAndroidPackages {
    # What the Android project of Tauri 2.12 asks for: `compileSdk = 37` for the app, 36 for
    # Tauri's own Android library (`:tauri-android`), and the build tools of its Android Gradle
    # plugin (9.3).
    platformVersions = [ "36" "37.0" ];
    buildToolsVersions = [ "36.0.0" "37.0.0" ];
    includeNDK = true;
    ndkVersions = [ ndkVersion ];
    includeEmulator = false;
    includeSystemImages = false;
    includeSources = false;
  };
  androidSdk = android.androidsdk;

  # The variables the Tauri CLI and Gradle read, set when the devShell starts (a setup hook of a
  # shell package is sourced by `nix develop`).
  android-env = pkgs.makeSetupHook { name = "holzi-android-env"; } (
    pkgs.writeText "holzi-android-env.sh" ''
      export ANDROID_HOME=${androidSdk}/libexec/android-sdk
      export ANDROID_SDK_ROOT="$ANDROID_HOME"
      export NDK_HOME="$ANDROID_HOME/ndk/${ndkVersion}"
      export ANDROID_NDK_ROOT="$NDK_HOME"
      export JAVA_HOME=${pkgs.jdk17.home}
    ''
  );

  # androidenv provides the SDK for x86_64 Linux and macOS hosts only.
  androidHost =
    (pkgs.stdenv.hostPlatform.isLinux && pkgs.stdenv.hostPlatform.isx86_64)
    || pkgs.stdenv.hostPlatform.isDarwin;
in
# WebKitGTK and Xvfb are Linux-only; the other platforms have their own drivers.
pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ tauri-driver webkit-webdriver ]
++ pkgs.lib.optionals androidHost [ androidSdk android-env ]
