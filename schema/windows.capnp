@0xfd5a69cd561508f1;

interface WindowsAgent {
  ping           @0 () -> ();
  getReport      @1 () -> (report :Report);
  setConfig      @2 (memoryIntervalMs :UInt64, cpuIntervalMs :UInt64) -> ();

  kill           @3 (pid :UInt32) -> (code :UInt32);
  suspend        @4 (pid :UInt32) -> (code :UInt32);
  resume         @5 (pid :UInt32) -> (code :UInt32);
  setPriority    @6 (pid :UInt32, priority :ProcessPriority) -> (code :UInt32);
  setAffinity    @7 (pid :UInt32, mask :UInt64) -> (code :UInt32);

  serviceStart   @8  (name :Text) -> (code :UInt32);
  serviceStop    @9  (name :Text) -> (code :UInt32);
  servicePause   @10 (name :Text) -> (code :UInt32);
  serviceResume  @11 (name :Text) -> (code :UInt32);
  serviceRestart @12 (name :Text) -> (code :UInt32);
}

enum ProcessPriority {
  idle        @0;
  belowNormal @1;
  normal      @2;
  aboveNormal @3;
  high        @4;
  realtime    @5;
}

enum SignatureStatus {
  unknown    @0;
  unsigned   @1;
  microsoft  @2;
  thirdParty @3;
}

struct Report {
  machine   @0 :MachineStats;
  processes @1 :List(ProcessStats);
  services  @2 :List(ServiceStats);
}

struct ServiceStats {
  name         @0 :Text;
  displayName  @1 :Text;
  pid          @2 :UInt32;
  state        @3 :ServiceState;
  loadGroup    @4 :Text;
  description  @5 :Text;

  # Path the SCM starts the service from, taken from the service config.
  # Needed to tell a Windows service from a third-party one; the app cannot
  # read it for every service because it runs unelevated.
  imagePath    @6 :Text;
}

enum ServiceState {
  unknown       @0;
  stopped       @1;
  startPending  @2;
  stopPending   @3;
  running       @4;
  continuePending @5;
  pausePending  @6;
  paused        @7;
}

struct MachineStats {
  totalPhysicalKb     @0 :UInt64;
  availablePhysicalKb @1 :UInt64;
  usedPhysicalKb      @2 :UInt64;
  cpuPercent          @3 :Float32;
  cpuMaxMhz           @4 :UInt64;
  cpuCurrentMhz       @5 :UInt64;

  diskReadBytes       @6 :UInt64;
  diskWriteBytes      @7 :UInt64;
  diskReadIops        @8 :UInt64;
  diskWriteIops       @9 :UInt64;

  netRxBytes          @10 :UInt64;
  netTxBytes          @11 :UInt64;
}

struct ProcessStats {
  pid                  @0 :UInt32;
  parentPid            @1 :UInt32;
  sessionId            @2 :UInt32;
  name                 @3 :Text;
  cmdline              @4 :List(Text);
  packageFullName      @5 :Text;
  packageRelativeAppId @6 :Text;
  cpuPercent           @7 :Float32;
  workingSetKb         @8 :UInt64;
  privateBytesKb       @9 :UInt64;
  peakWorkingSetKb     @10 :UInt64;

  diskReadBytes        @11 :UInt64;
  diskWriteBytes       @12 :UInt64;
  diskReadIops         @13 :UInt64;
  diskWriteIops        @14 :UInt64;

  netRxBytes           @15 :UInt64;
  netTxBytes           @16 :UInt64;

  isService            @17 :Bool;
  isKernelProcess      @18 :Bool;
  isWindowsProcess     @19 :Bool;
  signature            @20 :SignatureStatus;
  imagePath            @21 :Text;

  # Human-facing name: the executable's FileDescription, a packaged app's
  # manifest display name, or the shell's name for the file - whichever the
  # agent could resolve. Empty when none of them answered, in which case the
  # consumer should fall back to `name`.
  #
  # `name` stays exactly what the OS reports, so it remains usable for
  # matching and grouping; this field is only ever for display.
  displayName          @22 :Text;

  # The process's *private* working set: resident pages it does not share
  # with anyone. This is what Task Manager's Memory column shows, and the
  # only one of the three that can be summed - workingSetKb counts a shared
  # DLL once per process mapping it, so adding it up over a few hundred
  # processes reports more memory in use than the machine has.
  privateWorkingSetKb  @23 :UInt64;
}
