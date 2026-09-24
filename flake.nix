{
  description = "Reproducible Nix devShell skeleton, delivered by spaex";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        # Blanket allow, not a per-package predicate: `cudatoolkit` (pulled
        # in by consumers like `com.github.haexmas.atoms.holzi` for local
        # CUDA builds) is a meta-package bundling several separately
        # unfree-licensed sub-derivations (cuda_nvcc, cuda_cuobjdump, ...),
        # so scoping to one name is not enough and the set of names is not
        # stable across nixpkgs bumps. Revisit if a consumer ever needs
        # finer-grained control.
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
        };

        # spaex regenerates this from every currently-adopted molecule's
        # `nix_packages` fragment (spaex Spec 027's composable atom
        # category). Absent when no adopted molecule declares one — that
        # is a valid "no extra packages" state, not an error.
        generatedPackagesPath = ./.spaex/generated/nix-packages.json;
        packageNames =
          if builtins.pathExists generatedPackagesPath
          then builtins.fromJSON (builtins.readFile generatedPackagesPath)
          else [ ];
        # A plain name ("gtk3") indexes pkgs directly; a dotted name
        # ("gcc.cc.lib", "dbus.lib") walks nested attrs — needed because some
        # nixpkgs packages split their shared libraries into a non-default
        # output (e.g. plain `glib` resolves to its "bin" output, not the
        # one containing libglib-2.0.so; `dbus`'s library lives in `.lib`,
        # not its default "out").
        resolvePackage = name: pkgs.lib.getAttrFromPath (pkgs.lib.splitString "." name) pkgs;
        # Packages nixpkgs does not ship, or ships in a form that would clash with the
        # host, come from an optional file another molecule delivers:
        # `.devshell/packages.nix`, a function from `pkgs` to a list of
        # derivations. Read with the same guard as the list above, so no adopted
        # molecule delivering it is a valid state. The file must be tracked by git
        # like every other file a flake reads, and only one molecule can own that
        # path (an exclusive atom), so this is an extension point for one
        # contributor, not a composable category.
        extraPackagesPath = ./.devshell/packages.nix;
        extraPackages =
          if builtins.pathExists extraPackagesPath
          then import extraPackagesPath pkgs
          else [ ];
        packages = map resolvePackage packageNames ++ extraPackages;
      in
      {
        devShells.default = pkgs.mkShell {
          inherit packages;
          # Nix's own dynamic linker doesn't consult the host's ld.so.cache,
          # so anything the devShell's packages provide at runtime (not just
          # build time) needs to be on LD_LIBRARY_PATH explicitly — RPATH
          # alone isn't reliable here since Tauri's own build process
          # overwrites it with a bundle-relative convention. Generic by
          # construction: driven entirely by whatever nix_packages molecules
          # contribute, no per-package or per-consumer special-casing — with
          # the one exception below, which is itself gated on a contributed
          # package name.
          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath packages}:$LD_LIBRARY_PATH"
          '' + pkgs.lib.optionalString (builtins.elem "cudatoolkit" packageNames) ''
            # `cudarc` (pulled in by `mistralrs/cuda`) looks for one of
            # CUDA_HOME/CUDA_PATH/CUDA_ROOT/CUDA_TOOLKIT_ROOT_DIR at build time to
            # find -lcudart/-lnvrtc/-lcurand/-lcublas/-lcublasLt — unlike
            # LD_LIBRARY_PATH above, this is consulted by the *linker*, not the
            # dynamic loader, and without it cudarc falls back to host paths like
            # /usr/local/cuda that don't exist in this Nix-provided toolchain.
            # Gated on the contributed *name*, not on `pkgs.cudatoolkit` itself:
            # comparing derivations would evaluate cudatoolkit for every consumer,
            # and it does not evaluate on platforms nixpkgs does not support it on.
            export CUDA_ROOT="${pkgs.cudatoolkit}"
          '';
        };
      });
}
