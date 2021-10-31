{
  description = "Nagios/Icinga compatible plugin to search `journalctl` output for matching lines";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-compat = { url = "github:edolstra/flake-compat"; flake = false; };
  };

  outputs =
    inputs@{ self
    , nixpkgs
    , flake-utils
    , flake-compat
    }:
    { }
    //
    (flake-utils.lib.eachDefaultSystem
      (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            self.overlay
          ];
          config = { };
        };
      in
      rec {
        devShell = with pkgs; mkShell {
          buildInputs = [
            check_journal
          ];
        };
        defaultPackage = pkgs.check_journal;
      }
      )
    ) //
    {
      overlay = final: prev: {
        check_journal = prev.rustPlatform.buildRustPackage
          {
            pname = "check_journal";
            version = "1.2.0";

            src = ./.;

            cargoLock.lockFile = ./Cargo.lock;

            JOURNALCTL = "${prev.systemd}/bin/journalctl";

            nativeBuildInputs = with prev; [ ronn utillinux ];
            postBuild = "make man";

            preCheck = "patchShebangs fixtures/*.sh";

            postInstall = ''
              install -m 0644 -D -t $out/share/man/man1 man/check_journal.1
              install -m 0644 -D -t $out/share/doc/check_journal README.md
            '';
            meta.description = "Nagios/Icinga compatible plugin to search `journalctl` output for matching lines";
          };
      };
    };
}
