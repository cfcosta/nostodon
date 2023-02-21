{ config, lib, pkgs, ... }: {
  ec2.hvm = true;

  environment.systemPackages = with pkgs; [ nostodon ];

  networking.hostName = "nostodon-core-server";
  security.sudo.wheelNeedsPassword = false;

  nix.settings.trusted-users = [ "@wheel" ];

  users.users.nostodon = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];

    openssh.authorizedKeys.keys = [ ];
  };

  services.openssh = {
    enable = true;
    settings = {
      PermitRootLogin = lib.mkForce "no";
      PasswordAuthentication = false;
    };
  };

  systemd.services.nostodon = {
    enable = true;

    description = "Nostodon core server";

    unitConfig = {
      Type = "simple";
      Restart = "on-failure";
      User = "nostodon";
    };

    serviceConfig = {
      ExecStart = "${pkgs.nostodon}/bin/nostodon";

      # Hardening
      CapabilityBoundingSet = [ "" ];
      LockPersonality = true;
      PrivateTmp = true;
      ProcSubset = "pid";
      ProtectClock = true;
      ProtectControlGroups = true;
      ProtectHome = true;
      ProtectHostname = true;
      ProtectKernelLogs = true;
      ProtectKernelModules = true;
      ProtectKernelTunables = true;
      ProtectProc = "invisible";
      ProtectSystem = "strict";
      RestrictNamespaces = true;
      RestrictRealtime = true;
      RestrictSUIDSGID = true;
      SystemCallArchitectures = "native";
    };
  };

  system.stateVersion = "23.05";
}
