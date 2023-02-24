{ config, lib, pkgs, ... }:
let
  database = rec {
    host = "127.0.0.1";
    port = "5432";
    database = "nostodon";
    user = "nostodon";
    password = "nostodon";
    full-url = "postgres://${user}:${password}@${host}:${port}/${database}";
  };
in {
  imports = [ ./linode-base.nix ];

  environment.systemPackages = with pkgs; [ nostodon neovim ];

  networking.hostName = "nostodon-core-server";
  security.sudo.wheelNeedsPassword = false;

  nix.settings.trusted-users = [ "@wheel" ];

  users.users.nostodon = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];

    openssh.authorizedKeys.keys = [
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIKxNmAeczwJgH2GQ/qCYlIiV0M+QTqr/ZnISpT0TP90A cfcosta@mothership"
    ];
  };

  services.postgresql = {
    enable = true;
    ensureDatabases = [ database.database ];
    authentication = ''
      local   all             all                                     trust
      host    all             all             127.0.0.1/32            trust
      host    all             all             ::1/128                 trust
    '';
    ensureUsers = [{
      name = database.user;
      ensurePermissions = {
        "DATABASE ${database.database}" = "ALL PRIVILEGES";
      };
    }];
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

    requires = [ "postgresql.service" ];
    after = [ "postgresql.service" ];

    serviceConfig = {
      Type = "simple";
      Restart = "always";
      User = database.user;

      ExecStart =
        "${pkgs.nostodon}/bin/nostodon --database-url ${database.full-url}";

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
}
