let
  pkgs = import <nixpkgs> { };
in
pkgs.mkShell {
  nativeBuildInputs = [
    pkgs.cargo
    pkgs.cargo-watch
    pkgs.clippy
    pkgs.apacheHttpd  # For htpasswd
    pkgs.pkgconf
    pkgs.pre-commit
    pkgs.rustc
    pkgs.rustfmt
  ];
  buildInputs = [
    pkgs.openssl
  ];
}
