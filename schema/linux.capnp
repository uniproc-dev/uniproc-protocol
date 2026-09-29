@0xd0b00dd26d1a5151;

using Meta = import "meta.capnp";

# Semantic version of this protocol; see windows.capnp for the rules.
const version :Text = "2.1.0";

# Conventions, the same as windows.capnp's:
#
# - No data is a null pointer, an empty list or text, `unknown` in an enum, or
#   the type's maximum in a column row (0xFFFFFFFF, 0xFFFFFFFFFFFFFFFF). 0 is a
#   real value.
# - Cumulative counters go out raw. Rates and deltas are the consumer's, over
#   its own interval.
# - Bytes are bytes. Durations and CPU times are in 100 ns units.
# - `sampledAt` is CLOCK_BOOTTIME in 100 ns units: monotonic within one boot,
#   meaningful only as a difference.
# - A process is named by its pid together with its sequenceNumber: the
#   sequence number is the process's start time, which two processes can
#   share, but not two processes with the same pid. Rows, states, passports
#   and commands all carry both.
# - Counters the agent's own probes keep (cpuRunTime is not one of them) start
#   when the agent first sees a process, and start over when the agent
#   restarts; a delta across a reconnect to a new agent run is not a delta.
# - `pid` (and parentPid, and the pid in commands) is the kernel's global pid,
#   in the machine's initial pid namespace. On WSL no shell shows it: every
#   distro runs in a pid namespace of its own. localPid is the pid inside the
#   process's own namespace, the one ps there shows.
# - Kernel threads are not processes here.

interface LinuxAgent {
  # The agent returns `nonce` unchanged.
  ping             @0  (meta :Meta.RequestMeta, nonce :UInt64)
                      -> (meta :Meta.ResponseMeta, nonce :UInt64);

  # Conditional. The passports change when a process starts, exits or execs.
  # The etag of this answer is the `passportEtag` every other process list
  # refers to.
  getProcesses     @1  (meta :Meta.RequestMeta)
                      -> (meta :Meta.ResponseMeta, processes :List(ProcessInfo));

  # Conditional, with its own etag. `states` covers exactly the processes
  # getProcesses returns under `passportEtag`.
  getProcessStates @2  (meta :Meta.RequestMeta)
                      -> (meta :Meta.ResponseMeta, passportEtag :UInt64, states :List(ProcessState));

  # Conditional: the namespaces processes run in, and the docker containers
  # behind some of them.
  getEnvironments  @3  (meta :Meta.RequestMeta)
                      -> (meta :Meta.ResponseMeta, environments :List(EnvironmentInfo),
                          dockerContainers :List(DockerContainerInfo));

  # Pushes snapshots to `listener` until `handle` is released or the
  # connection goes away. The first update carries the full lists; every
  # later one carries what moved since the update before, and its sample
  # refers to the lists as they stand after that update. One call is in
  # flight at a time: while the listener has not returned, newer samples
  # replace older ones and list changes accumulate, so a slow listener gets
  # fewer calls, never a gap. Paced at spec.intervalMs, clamped to
  # [100, 60000], 0 meaning 1000. The agent samples the union of all live
  # specs at the shortest interval and gives each listener its own metrics
  # only. An error returned by the listener ends the watch. To change the
  # spec, release the handle and watch again.
  watch            @4  (meta :Meta.RequestMeta, spec :MetricSpec, listener :AgentListener)
                      -> (meta :Meta.ResponseMeta, handle :WatchHandle);

  # Process commands. `sequenceNumber` must be the process's, so a pid reused
  # since the caller saw it is never hit; 0 skips the check. The agent
  # compares at the kernel's USER_HZ resolution, which pid reuse cannot beat.
  # `code` is 0 or an errno: 3 (ESRCH) when the process is gone or the
  # sequence number does not match, 1 (EPERM) or 13 (EACCES) when the agent
  # may not. A process outside the agent's own pid namespace (another
  # distro's) cannot be acted on: 1 (EPERM).
  kill             @5  (meta :Meta.RequestMeta, pid :UInt32, sequenceNumber :UInt64)
                      -> (meta :Meta.ResponseMeta, code :UInt32);
  terminate        @6  (meta :Meta.RequestMeta, pid :UInt32, sequenceNumber :UInt64)
                      -> (meta :Meta.ResponseMeta, code :UInt32);
  suspend          @7  (meta :Meta.RequestMeta, pid :UInt32, sequenceNumber :UInt64)
                      -> (meta :Meta.ResponseMeta, code :UInt32);
  resume           @8  (meta :Meta.RequestMeta, pid :UInt32, sequenceNumber :UInt64)
                      -> (meta :Meta.ResponseMeta, code :UInt32);
  # Any signal by number; kill, terminate, suspend and resume are 9, 15, 19
  # and 18.
  signal           @9  (meta :Meta.RequestMeta, pid :UInt32, sequenceNumber :UInt64, signal :UInt32)
                      -> (meta :Meta.ResponseMeta, code :UInt32);
  # Applies to every thread of the process. nice is -20..19.
  setNice          @10 (meta :Meta.RequestMeta, pid :UInt32, sequenceNumber :UInt64, nice :Int32)
                      -> (meta :Meta.ResponseMeta, code :UInt32);
  # Applies to every thread. Bit n of word n / 64 allows CPU n.
  setAffinity      @11 (meta :Meta.RequestMeta, pid :UInt32, sequenceNumber :UInt64, mask :List(UInt64))
                      -> (meta :Meta.ResponseMeta, code :UInt32);

  # Conditional: the units of the system manager (systemd, pid 1 of the
  # agent's distro), every loaded one and every installed unit file that is
  # not loaded, templates excepted. Moves when a unit's state, job or main
  # process changes, and when unit files change. Empty when the distro does
  # not run systemd.
  getUnits         @12 (meta :Meta.RequestMeta)
                      -> (meta :Meta.ResponseMeta, units :List(UnitInfo));

  # Unit commands queue a job with mode "replace", as systemctl does, and
  # answer once it is queued; how it ends shows in the unit's state. `name`
  # is the full unit name, suffix included. `code` is 0 or an errno:
  # 2 (ENOENT) no such unit, 13 (EACCES) not permitted, 16 (EBUSY) another
  # command for the same unit is still being handed to systemd,
  # 53 (EBADR) the job does not apply to the unit, as reload to a unit that
  # cannot reload, 132 (ERFKILL) the unit is masked, 35 (EDEADLK) the job
  # conflicts with jobs queued already, 107 (ENOTCONN) the agent has no
  # connection to systemd, 5 (EIO) anything else.
  unitStart        @13 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  unitStop         @14 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  unitRestart      @15 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  unitReload       @16 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);

  # Calls watcher.changed with the unit's status now, then on every change
  # until `handle` is released. watcher.ended is called when following
  # stops: no unit or unit file has the name, the unit was unloaded and its
  # file removed, or the agent is stopping.
  watchUnit        @17 (meta :Meta.RequestMeta, name :Text, watcher :UnitWatcher)
                      -> (meta :Meta.ResponseMeta, handle :WatchHandle);
}

# Implemented by the client and called by the agent. The agent sends `meta` empty.
interface AgentListener {
  update @0 (meta :Meta.RequestMeta, lists :ListsUpdate,
             processes :ProcessColumns, machine :MachineSample) -> ();

  # The watch stopped for good: the agent is stopping.
  ended  @1 (meta :Meta.RequestMeta) -> ();
}

# Released by the client to stop watching; it has no methods.
interface WatchHandle {}

# What changed in the conditional lists since the previous update. Rows are
# keyed by pid and sequenceNumber. The etags are always set and name the lists
# after this update: `passportEtag` is the one `processes` in the same update
# refers to.
struct ListsUpdate {
  passports :union {
    unchanged @0 :Void;
    full      @1 :List(ProcessInfo);
    delta     @2 :PassportsDelta;
  }
  passportEtag @3 :UInt64;

  states :union {
    unchanged @4 :Void;
    full      @5 :List(ProcessState);
    delta     @6 :StatesDelta;
  }
  statesEtag @7 :UInt64;

  environments :union {
    unchanged @8 :Void;
    full      @9 :Environments;
  }
  environmentsEtag @10 :UInt64;

  # The list getUnits answers, under the same etag.
  units :union {
    unchanged @11 :Void;
    full      @12 :List(UnitInfo);
  }
  unitsEtag @13 :UInt64;
}

struct Environments {
  environments     @0 :List(EnvironmentInfo);
  dockerContainers @1 :List(DockerContainerInfo);
}

# Applies to the passports under `baseEtag`. A client holding another etag
# resyncs through getProcesses, or releases the watch and watches again.
struct PassportsDelta {
  baseEtag @0 :UInt64;
  # Processes that exited. Changes accumulated while the listener was busy
  # may name a process the client never received; ignore it.
  left     @1 :List(ProcessKey);
  # Processes that started, or execed and so have a new passport, as whole rows.
  upserted @2 :List(ProcessInfo);
}

# Applies to the states under `baseEtag`, like PassportsDelta.
struct StatesDelta {
  baseEtag @0 :UInt64;
  left     @1 :List(ProcessKey);
  upserted @2 :List(ProcessState);
}

struct ProcessKey {
  pid            @0 :UInt32;
  sequenceNumber @1 :UInt64;
}

# What one watch wants.
struct MetricSpec {
  intervalMs @0 :UInt32;
  processes  @1 :List(ProcessMetric);
  machine    @2 :List(MachineMetric);
}

# One value per column, in ProcessColumns order.
enum ProcessMetric {
  cpuUserTime                @0;
  cpuKernelTime              @1;
  cpuRunTime                 @2;

  residentSet                @3;
  residentAnon               @4;
  residentFile               @5;
  residentShmem              @6;
  peakResidentSet            @7;
  virtualSize                @8;
  peakVirtualSize            @9;
  swap                       @10;
  minorFaults                @11;
  majorFaults                @12;

  threads                    @13;
  voluntaryContextSwitches   @14;
  involuntaryContextSwitches @15;

  ioReadOps                  @16;
  ioWriteOps                 @17;
  ioReadBytes                @18;
  ioWriteBytes               @19;
  diskReadBytes              @20;
  diskWriteBytes             @21;

  fileReadOps                @22;
  fileWriteOps               @23;
  fileReadBytes              @24;
  fileWriteBytes             @25;
  pipeReadBytes              @26;
  pipeWriteBytes             @27;
  sendfileBytes              @28;

  netRxBytes                 @29;
  netTxBytes                 @30;
  transports                 @31;
}

# Process metrics as columns: `pids`, `sequenceNumbers` and every non-null
# column have the same length, and row i of every column belongs to the same
# process. A column is null when it was not requested or the agent has no data
# for it.
struct ProcessColumns {
  snapshot        @0  :UInt64;
  sampledAt       @1  :UInt64;
  # The getProcesses etag these rows were sampled against, as in windows.capnp.
  passportEtag    @2  :UInt64;
  pids            @3  :List(UInt32);
  sequenceNumbers @4  :List(UInt64);

  # Cumulative, 100 ns, summed over the process's threads, exited ones
  # included, since the process started. User and kernel time are the
  # kernel's tick-sampled split; cpuRunTime is the scheduler's exact runtime.
  cpuUserTime     @5  :List(UInt64);
  cpuKernelTime   @6  :List(UInt64);
  cpuRunTime      @7  :List(UInt64);

  # Bytes, current. residentSet is residentAnon + residentFile + residentShmem.
  residentSet     @8  :List(UInt64);
  residentAnon    @9  :List(UInt64);
  residentFile    @10 :List(UInt64);
  residentShmem   @11 :List(UInt64);
  peakResidentSet @12 :List(UInt64);
  virtualSize     @13 :List(UInt64);
  peakVirtualSize @14 :List(UInt64);
  swap            @15 :List(UInt64);
  # Cumulative, over all threads, exited ones included.
  minorFaults     @16 :List(UInt64);
  majorFaults     @17 :List(UInt64);

  threads                    @18 :List(UInt32);
  # Cumulative, over all threads, exited ones included.
  voluntaryContextSwitches   @19 :List(UInt64);
  involuntaryContextSwitches @20 :List(UInt64);

  # The kernel's I/O accounting (/proc/<pid>/io), cumulative, exited threads
  # included. io* is every read and write syscall, whatever it hit; disk* is
  # what reached the block layer.
  ioReadOps       @21 :List(UInt64);
  ioWriteOps      @22 :List(UInt64);
  ioReadBytes     @23 :List(UInt64);
  ioWriteBytes    @24 :List(UInt64);
  diskReadBytes   @25 :List(UInt64);
  diskWriteBytes  @26 :List(UInt64);

  # Counted by the agent's probes from when it first saw the process, and
  # from zero again when the agent restarts; cumulative within one agent run.
  # file* is reads and writes on regular files, pipe* on pipes, sendfile the
  # bytes sendfile(2) moved.
  fileReadOps     @27 :List(UInt64);
  fileWriteOps    @28 :List(UInt64);
  fileReadBytes   @29 :List(UInt64);
  fileWriteBytes  @30 :List(UInt64);
  pipeReadBytes   @31 :List(UInt64);
  pipeWriteBytes  @32 :List(UInt64);
  sendfileBytes   @33 :List(UInt64);

  # TCP and UDP payload, loopback and remote together, counted like file*.
  # The split is in `transports`.
  netRxBytes      @34 :List(UInt64);
  netTxBytes      @35 :List(UInt64);

  # One entry per row.
  transports      @36 :List(Transports);
}

# Socket payload by transport, counted by the agent's probes: for a process
# from when the agent first saw it, for the machine from when the agent
# started, and from zero again when the agent restarts. Loopback is traffic
# whose peer is on this machine. vsock is the channel to the Windows host;
# 9p is WSL's file sharing with it.
struct Transports {
  tcpLoopbackRx @0  :UInt64;
  tcpLoopbackTx @1  :UInt64;
  tcpRemoteRx   @2  :UInt64;
  tcpRemoteTx   @3  :UInt64;
  udpLoopbackRx @4  :UInt64;
  udpLoopbackTx @5  :UInt64;
  udpRemoteRx   @6  :UInt64;
  udpRemoteTx   @7  :UInt64;
  unixRx        @8  :UInt64;
  unixTx        @9  :UInt64;
  vsockRx       @10 :UInt64;
  vsockTx       @11 :UInt64;
  p9Rx          @12 :UInt64;
  p9Tx          @13 :UInt64;
}

# Machine data comes in groups; a group is null when it was not requested or
# the agent has no data for it.
enum MachineMetric {
  cpu             @0;
  memory          @1;
  disk            @2;
  network         @3;
  processors      @4;
  networkAdapters @5;
  load            @6;
}

struct MachineSample {
  snapshot        @0 :UInt64;
  sampledAt       @1 :UInt64;
  cpu             @2 :MachineCpu;
  memory          @3 :MachineMemory;
  disk            @4 :MachineDisk;
  network         @5 :MachineNetwork;
  # One entry per online CPU, in CPU number order. Their sums are MachineCpu's
  # times.
  processors      @6 :List(MachineProcessor);
  # Every interface but loopback.
  networkAdapters @7 :List(NetworkAdapter);
  load            @8 :MachineLoad;
}

# Cumulative, 100 ns, as /proc/stat keeps them. guestTime is already counted
# in userTime and guestNiceTime in niceTime, so the total is user + nice +
# system + idle + iowait + irq + softirq + steal, and busy time is that less
# idle and iowait.
struct MachineCpu {
  userTime      @0  :UInt64;
  niceTime      @1  :UInt64;
  systemTime    @2  :UInt64;
  idleTime      @3  :UInt64;
  iowaitTime    @4  :UInt64;
  irqTime       @5  :UInt64;
  softirqTime   @6  :UInt64;
  stealTime     @7  :UInt64;
  guestTime     @8  :UInt64;
  guestNiceTime @9  :UInt64;
  # Online logical CPUs.
  count         @10 :UInt32;
  # The fastest CPU's current clock, 0 when the kernel does not tell.
  currentMhz    @11 :UInt32;
}

struct MachineProcessor {
  userTime      @0 :UInt64;
  niceTime      @1 :UInt64;
  systemTime    @2 :UInt64;
  idleTime      @3 :UInt64;
  iowaitTime    @4 :UInt64;
  irqTime       @5 :UInt64;
  softirqTime   @6 :UInt64;
  stealTime     @7 :UInt64;
}

# Bytes, current, from /proc/meminfo.
struct MachineMemory {
  total       @0 :UInt64;
  free        @1 :UInt64;
  available   @2 :UInt64;
  buffers     @3 :UInt64;
  cached      @4 :UInt64;
  shmem       @5 :UInt64;
  swapTotal   @6 :UInt64;
  swapFree    @7 :UInt64;
  # The commit limit and what is committed now (Committed_AS).
  commitLimit @8 :UInt64;
  committed   @9 :UInt64;
}

# Whole disks, partitions left out, cumulative, from /proc/diskstats.
struct MachineDisk {
  readOps    @0 :UInt64;
  writeOps   @1 :UInt64;
  readBytes  @2 :UInt64;
  writeBytes @3 :UInt64;
  # 100 ns the disks spent with I/O in flight, summed over disks.
  busyTime   @4 :UInt64;
}

# Socket payload over the whole machine since the agent started, from zero
# again when it restarts. rxBytes and txBytes are TCP and UDP, loopback and
# remote together.
struct MachineNetwork {
  rxBytes    @0 :UInt64;
  txBytes    @1 :UInt64;
  transports @2 :Transports;
}

struct NetworkAdapter {
  name       @0 :Text;
  # Backed by a device rather than virtual (bridges, veth pairs, tunnels).
  hardware   @1 :Bool;
  # Bits per second; 0 when the driver does not report it.
  linkSpeed  @2 :UInt64;
  up         @3 :Bool;
  # Cumulative, as the interface counts them, headers included.
  rxBytes    @4 :UInt64;
  txBytes    @5 :UInt64;
  rxPackets  @6 :UInt64;
  txPackets  @7 :UInt64;
}

struct MachineLoad {
  # The kernel's load averages, times 100.
  load1   @0 :UInt32;
  load5   @1 :UInt32;
  load15  @2 :UInt32;
  running @3 :UInt32;
  tasks   @4 :UInt32;
}

# What a process is: fixed until it execs, so it travels through the
# conditional getProcesses.
struct ProcessInfo {
  pid            @0  :UInt32;
  parentPid      @1  :UInt32;
  # The process's start time, CLOCK_BOOTTIME in ns, unchanged by exec. With
  # the pid it names the process within one boot; alone it may repeat.
  sequenceNumber @2  :UInt64;
  # Wall clock, 100 ns since 1970-01-01 UTC.
  startTime      @3  :UInt64;
  # The kernel's task name (comm), at most 15 bytes.
  name           @4  :Text;
  exePath        @5  :Text;
  cmdline        @6  :List(Text);
  uid            @7  :UInt32;
  # uid resolved against the agent's own user database; empty when it has no
  # entry, as for a user that exists only inside a container.
  user           @8  :Text;
  # The pid inside the process's own pid namespace.
  localPid       @9  :UInt32;
  mntNs          @10 :UInt64;
  pidNs          @11 :UInt64;
  # The cgroup v2 path, as /proc/<pid>/cgroup gives it.
  cgroup         @12 :Text;
}

# How a process is running right now. Changes rarely, so it travels through
# the conditional getProcessStates.
struct ProcessState {
  pid            @0 :UInt32;
  sequenceNumber @1 :UInt64;
  # The main thread's state.
  state          @2 :TaskState;
  nice           @3 :Int32;
  policy         @4 :SchedPolicy;
  # 1..99 under fifo and rr, 0 otherwise.
  rtPriority     @5 :UInt32;
}

enum TaskState {
  unknown     @0;
  running     @1;
  sleeping    @2;
  diskSleep   @3;
  stopped     @4;
  tracingStop @5;
  zombie      @6;
  idle        @7;
}

enum SchedPolicy {
  unknown  @0;
  other    @1;
  fifo     @2;
  rr       @3;
  batch    @4;
  idle     @5;
  deadline @6;
}

# A mount namespace some process runs in.
struct EnvironmentInfo {
  mntNs @0 :UInt64;
  pidNs @1 :UInt64;
  kind  @2 :EnvironmentKind;
  # The distro's PRETTY_NAME for currentDistro, the container id for
  # dockerContainer, empty otherwise.
  name  @3 :Text;
}

enum EnvironmentKind {
  unknown                  @0;
  currentDistro            @1;
  dockerContainer          @2;
  unknownExternalNamespace @3;
}

struct DockerContainerInfo {
  id         @0 :Text;
  mntNs      @1 :UInt64;
  pidNs      @2 :UInt64;
  apiVersion @3 :Text;
  # The Engine API's inspect answer for the container, verbatim.
  rawJson    @4 :Text;
}

# Implemented by the client and called by the agent. The agent sends `meta` empty.
interface UnitWatcher {
  changed @0 (meta :Meta.RequestMeta, status :UnitStatus) -> ();
  ended   @1 (meta :Meta.RequestMeta) -> ();
}

struct UnitInfo {
  # The full unit name, "ssh.service"; its suffix is the unit type.
  name               @0 :Text;
  description        @1 :Text;
  loadState          @2 :UnitLoadState;
  activeState        @3 :UnitActiveState;
  # The type's own finer state, as systemctl shows it: "running", "exited",
  # "dead", "listening", "mounted", "waiting"...
  subState           @4 :Text;
  # How the unit file is installed, when there is one.
  unitFileState      @5 :UnitFileState;
  # The installed unit file named after the unit, as systemctl
  # list-unit-files gives it. Empty for a unit without one: a device, a
  # scope, a transient unit, a mount from fstab.
  unitFile           @6 :Text;
  # A service's main process, as ProcessInfo names it; 0 and 0 when it has
  # none, and for other unit types.
  mainPid            @7 :UInt32;
  mainSequenceNumber @8 :UInt64;
  # The job queued for the unit, `none` when there is none.
  job                @9 :UnitJob;
}

# What watchUnit tells about one unit.
struct UnitStatus {
  loadState          @0 :UnitLoadState;
  activeState        @1 :UnitActiveState;
  subState           @2 :Text;
  mainPid            @3 :UInt32;
  mainSequenceNumber @4 :UInt64;
  job                @5 :UnitJob;
  # A service's result of its last run: "success", "exit-code", "signal",
  # "core-dump", "timeout", "watchdog", "start-limit-hit"... Empty for other
  # unit types.
  result             @6 :Text;
  # How the service's main process last ended, as waitid reports it:
  # 1 (CLD_EXITED) with the exit status in execMainStatus, 2 (CLD_KILLED) or
  # 3 (CLD_DUMPED) with the signal number. 0 while it has not ended.
  execMainCode       @7 :UInt32;
  execMainStatus     @8 :Int32;
  # Times systemd restarted the service by its Restart= setting.
  restarts           @9 :UInt32;
}

enum UnitLoadState {
  unknown     @0;
  loaded      @1;
  notFound    @2;
  badSetting  @3;
  error       @4;
  merged      @5;
  masked      @6;
  stub        @7;
  # An installed unit file systemd has not loaded: nothing needs the unit
  # now. It loads when a command or a dependency asks for it.
  unloaded    @8;
}

enum UnitActiveState {
  unknown      @0;
  active       @1;
  reloading    @2;
  inactive     @3;
  failed       @4;
  activating   @5;
  deactivating @6;
  maintenance  @7;
  refreshing   @8;
}

enum UnitFileState {
  unknown        @0;
  enabled        @1;
  enabledRuntime @2;
  linked         @3;
  linkedRuntime  @4;
  alias          @5;
  masked         @6;
  maskedRuntime  @7;
  static         @8;
  disabled       @9;
  indirect       @10;
  generated      @11;
  transient      @12;
  bad            @13;
}

enum UnitJob {
  unknown       @0;
  none          @1;
  start         @2;
  verifyActive  @3;
  stop          @4;
  reload        @5;
  restart       @6;
  tryRestart    @7;
  tryReload     @8;
  reloadOrStart @9;
}
