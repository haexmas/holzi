{
  description = "Reproducible Nix devShell skeleton, delivered by spaex";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # spaex regenerates this from every currently-adopted molecule's
        # `nix_packages` fragment (spaex Spec 027's composable atom
        # category). Absent when no adopted molecule declares one — that
        # is a valid "no extra packages" state, not an error.
        generatedPackagesPath = ./.spaex/generated/nix-packages.json;
        packageNames =
          if builtins.pathExists generatedPackagesPath
          then builtins.fromJSON (builtins.readFile generatedPackagesPath)
          else [ ];
        packages = map (name: pkgs.${name}) packageNames;
      in
      {
        devShells.default = pkgs.mkShell {
          inherit packages;
        };
      });
}
