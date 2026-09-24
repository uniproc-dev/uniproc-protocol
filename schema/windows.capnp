@0xfd5a69cd561508f1;

using Meta = import "meta.capnp";

interface WindowsAgent {
  # The agent returns `nonce` unchanged. A client puts the request's number
  # there and compares, so a reply that reached the wrong call is caught.
  ping              @0  (meta :Meta.RequestMeta, nonce :UInt64)
                       -> (meta :Meta.ResponseMeta, nonce :UInt64);
  setConfig         @1  (meta :Meta.RequestMeta, memoryIntervalMs :UInt64, cpuIntervalMs :UInt64)
                       -> (meta :Meta.ResponseMeta);

  getMachine        @2  (meta :Meta.RequestMeta) -> (meta :Meta.ResponseMeta, machine :MachineStats);

  # Conditional: answers notModified while the inventory is unchanged.
  getServices       @3  (meta :Meta.RequestMeta)
                       -> (meta :Meta.ResponseMeta, services :List(ServiceStats));

  # Conditional: the set of processes and their static details only changes
  # when a process starts or exits, so this answers notModified most of the time.
  getProcesses      @4  (meta :Meta.RequestMeta)
                       -> (meta :Meta.ResponseMeta, processes :List(ProcessInfo));

  # Always fresh. `processesEtag` is the getProcesses etag these metrics were
  # sampled against, and `metrics` covers exactly the pids that getProcesses
  # returns under that etag. A caller holding a different etag must refetch
  # getProcesses before joining by pid, or a reused pid lands on the wrong name.
  #
  # The processes etag also moves when a service starts or stops, since
  # ProcessInfo.isService is derived from the service inventory.
  getProcessMetrics @5  (meta :Meta.RequestMeta)
                       -> (meta :Meta.ResponseMeta, processesEtag :UInt64, metrics :List(ProcessMetrics));

  kill              @6  (meta :Meta.RequestMeta, pid :UInt32) -> (meta :Meta.ResponseMeta, code :UInt32);
  suspend           @7  (meta :Meta.RequestMeta, pid :UInt32) -> (meta :Meta.ResponseMeta, code :UInt32);
  resume            @8  (meta :Meta.RequestMeta, pid :UInt32) -> (meta :Meta.ResponseMeta, code :UInt32);
  setPriority       @9  (meta :Meta.RequestMeta, pid :UInt32, priority :ProcessPriority)
                       -> (meta :Meta.ResponseMeta, code :UInt32);
  setAffinity       @10 (meta :Meta.RequestMeta, pid :UInt32, mask :UInt64)
                       -> (meta :Meta.ResponseMeta, code :UInt32);

  serviceStart      @11 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  serviceStop       @12 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  servicePause      @13 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  serviceResume     @14 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  serviceRestart    @15 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
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

  cpuInterruptPercent @12 :Float32;
  cpuDpcPercent       @13 :Float32;
}

# What a process is. Fixed for its lifetime, so it travels only through the
# conditional getProcesses.
struct ProcessInfo {
  pid                  @0 :UInt32;
  parentPid            @1 :UInt32;
  sessionId            @2 :UInt32;
  name                 @3 :Text;
  cmdline              @4 :List(Text);
  packageFullName      @5 :Text;
  packageRelativeAppId @6 :Text;

  isService            @7 :Bool;
  isKernelProcess      @8 :Bool;
  isWindowsProcess     @9 :Bool;
  signature            @10 :SignatureStatus;
  imagePath            @11 :Text;

  # Human-facing name: the executable's FileDescription, a packaged app's
  # manifest display name, or the shell's name for the file - whichever the
  # agent could resolve. Empty when none of them answered, in which case the
  # consumer should fall back to `name`.
  #
  # `name` stays exactly what the OS reports, so it remains usable for
  # matching and grouping; this field is only ever for display.
  displayName          @12 :Text;

  # Pid of the conhost (conhost.exe or OpenConsole.exe) serving this process's
  # console, as of enrichment, or 0 when there is none. Taken from
  # NtQueryInformationProcess(ProcessConsoleHostProcess) only when the low two
  # bits of the value equal 1; with any other flag the high bits hold something
  # else (usually the parent pid) and this field is 0. A later
  # FreeConsole/AttachConsole is not reflected.
  consoleHostPid       @13 :UInt32;
}

# What a process is doing right now. Numbers only, joined to ProcessInfo by pid.
struct ProcessMetrics {
  pid                  @0 :UInt32;
  cpuPercent           @1 :Float32;
  workingSetKb         @2 :UInt64;
  privateBytesKb       @3 :UInt64;
  peakWorkingSetKb     @4 :UInt64;

  # The process's *private* working set: resident pages it does not share
  # with anyone. This is what Task Manager's Memory column shows, and the
  # only one of the three that can be summed - workingSetKb counts a shared
  # DLL once per process mapping it, so adding it up over a few hundred
  # processes reports more memory in use than the machine has.
  privateWorkingSetKb  @5 :UInt64;

  diskReadBytes        @6 :UInt64;
  diskWriteBytes       @7 :UInt64;
  diskReadIops         @8 :UInt64;
  diskWriteIops        @9 :UInt64;

  netRxBytes           @10 :UInt64;
  netTxBytes           @11 :UInt64;
}
