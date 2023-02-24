{ config, lib, pkgs, modulesPath, ... }: {
  imports = [ (modulesPath + "/profiles/qemu-guest.nix") ];

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
  hardware.cpu.amd.updateMicrocode =
    lib.mkDefault config.hardware.enableRedistributableFirmware;

  boot.initrd.availableKernelModules =
    [ "virtio_pci" "virtio_scsi" "ahci" "sd_mod" ];
  boot.initrd.kernelModules = [ ];
  boot.kernelModules = [ ];
  boot.extraModulePackages = [ ];

  fileSystems."/" = {
    device = "/dev/sda";
    fsType = "ext4";
  };

  swapDevices = [{ device = "/dev/sdb"; }];

  boot.loader.timeout = 10;
  boot.loader.grub = {
    enable = true;
    version = 2;
    forceInstall = true;

    # For LISH support
    device = "nodev";
    extraConfig = ''
      serial --speed=19200 --unit=0 --word=8 --parity=no --stop=1;
      terminal_input serial;
      terminal_output serial
    '';
  };

  # Set up LISH console
  boot.kernelParams = [ "console=ttyS0,19200n8" ];

  networking = {
    # Make things compatible with normal network configuration on linode
    usePredictableInterfaceNames = false;

    # Disable DHCP globally as we will not need it.
    useDHCP = false;

    # Enable it for eth0
    interfaces.eth0.useDHCP = true;
  };

  # Latest :)
  system.stateVersion = "23.05";
}
