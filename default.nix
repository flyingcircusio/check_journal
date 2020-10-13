# Build expression for NixOS 20.03
{ pkgs ? import <nixpkgs> {} }:

with pkgs.lib;
with pkgs.rustPlatform;

buildRustPackage rec {
  name = "check-journal-${version}";
  version = "1.2.0-dev";

  src = cleanSourceWith {
    filter = n: t: baseNameOf n != "target";
    src = cleanSource ./.;
  };
  cargoSha256 = "08mmfgnlh9nw21x5pkw6wq2lbm438zz58q84h7rrliacn743lmvy";

  # used in src/main.rs to set default path for journalctl
  JOURNALCTL = "${pkgs.systemd}/bin/journalctl";

  nativeBuildInputs = with pkgs; [ ronn utillinux ];
  postBuild = "make man";

  preCheck = "patchShebangs fixtures/journalctl-cursor-file.sh";

  postInstall = ''
    install -m 0644 -D -t $out/share/man/man1 man/check_journal.1
    install -m 0644 -D -t $out/share/doc/check_journal README.md
  '';

  meta = {
    description = "Nagios/Icinga compatible plugin to search `journalctl` " +
      "output for matching lines.";
    homepage = "https://github.com/flyingcircusio/check_journal";
    maintainer = with maintainers; [ ckauhaus ];
    license = with licenses; [ bsd3 ];
  };
}
