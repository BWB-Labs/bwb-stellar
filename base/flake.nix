{
  description = "Foundry + Solidity + Node + Safe CLI dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          foundry = pkgs.foundry-bin or pkgs.foundry;
          python = pkgs.python312;
        in
        {
          default = pkgs.mkShell {
            packages = [
              foundry
              pkgs.solc
              pkgs.nodejs_22
              pkgs.nodePackages.typescript
              python
              python.pkgs.pip
              python.pkgs.virtualenv
              pkgs.jq
            ];

            shellHook = ''
              export VENV_DIR=.venv

              if [ ! -d "$VENV_DIR" ]; then
                python -m venv "$VENV_DIR"
              fi

              source "$VENV_DIR/bin/activate"

              if ! command -v safe-cli >/dev/null 2>&1; then
                pip install -U pip wheel
                pip install -U safe-cli
              fi

              echo "Foundry + Node + Safe CLI dev shell"
              echo "Commands: forge, cast, anvil, chisel, node, npm, tsc, safe-cli"
            '';
          };
        }
      );
    };
}
