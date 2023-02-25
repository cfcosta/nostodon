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

  # TODO: Move debugging tools into a flag
  environment.systemPackages = with pkgs; [ nostodon neovim htop tmux ];

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

      # Unit Sandboxing and Hardening
      # Service has it's own unshared tmpfs
      PrivateTmp = true;
      # Service can not see or change real devices
      PrivateDevices = true;
      # No capabilities by default
      CapabilityBoundingSet = [ "" ];
      AmbientCapabilities = [ "" ];
      # Protect the following from modification:
      # - The entire filesystem
      # - sysctl settings and loaded kernel modules
      # - No modifications allowed to Control Groups
      # - Hostname
      # - System Clock
      ProtectSystem = "strict";
      ProtectKernelTunables = true;
      ProtectKernelModules = true;
      ProtectControlGroups = true;
      ProtectClock = true;
      ProtectHostname = true;
      # Prevent access to the following:
      # - /home directory
      # - Kernel logs
      ProtectHome = "tmpfs";
      ProtectKernelLogs = true;
      # Make sure that the process can only see PIDs and process details of itself,
      # and the second option disables seeing details of things like system load and
      # I/O etc
      ProtectProc = "invisible";
      ProcSubset = "pid";
      # While not needed, we set these options explicitly
      # - This process has been given access to the host network
      # - It can also communicate with any IP Address
      PrivateNetwork = false;
      RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" ];
      IPAddressAllow = "any";
      # Restrict system calls
      SystemCallArchitectures = "native";
      SystemCallFilter = [ "@system-service" "~@privileged @resources" ];
      # Misc restrictions
      RestrictSUIDSGID = true;
      RemoveIPC = true;
      NoNewPrivileges = true;
      RestrictRealtime = true;
      RestrictNamespaces = true;
      LockPersonality = true;
      PrivateUsers = true;
      # Disable this if the application runs an or inside an interpreter
      MemoryDenyWriteExecute = true;
    };
  };
}
