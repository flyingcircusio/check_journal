# Build expression for NixOS 20.03
{ pkgs ? import <nixpkgs> {} }:

with pkgs.lib;
with pkgs.rustPlatform;

buildRustPackage rec {
  name = "check-journal-${version}";
  version = "1.1.3-dev";

  src = cleanSourceWith {
    filter = n: t: baseNameOf n != "target";
    src = cleanSource ./.;
  };
  cargoSha256 = "1lyz2r5nrfhnrw3lkqh7zq2cqmh8mrvavyv0bfxlvxki36prfczc";
  JOURNALCTL = "${pkgs.systemd}/bin/journalctl";

  nativeBuildInputs = with pkgs; [ ronn ];
  postBuild = "make doc";
  postInstall = ''
    install -D check_journal.1 $out/share/man/man1/check_journal.1
    install -D README.md $out/share/doc/check_journal/README.md
  '';

  meta = {
    description = "Nagios/Icinga compatible plugin to search `journalctl` " +
      "output for matching lines.";
    homepage = "https://github.com/flyingcircusio/check_journal";
    maintainer = with maintainers; [ ckauhaus ];
    license = with licenses; [ bsd3 ];
  };
}
