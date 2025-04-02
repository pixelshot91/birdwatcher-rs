{ self, pkgs }:

pkgs.nixosTest {
  name = "birdwatcher-rs integration test";
  nodes.machine =
    { config, pkgs, ... }:
    {
      # a = 42;
      imports = [
        self.nixosModules.birdwatcher-rs-NixosModule
      ];
      services.birdwatcher-rs = {
        enable = true;
      };

      system.stateVersion = "23.11";
    };

  testScript = ''
    machine.wait_for_unit("birdwatcher-rs.service")
    machine.screenshot("my_screen_1.png")
    machine.wait_for_open_port(3000, 'localhost', 2)
  '';
}
