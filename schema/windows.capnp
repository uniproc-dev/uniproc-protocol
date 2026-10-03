@0xfd5a69cd561508f1;

using Meta = import "meta.capnp";

# Semantic version of this protocol, sent in the handshake next to the file id.
# Appending fields, methods or enumerants is a minor bump; anything else that
# changes the wire is a major one, and peers on different majors refuse each
# other. tests/wire_compat.rs holds every change against the last tag.
#
# An appended value an older reader cannot decode, a union member, an
# enumerant or a ResponseStatus, is sent only to a peer whose handshake
# version has it. A reader that meets one anyway treats an enumerant as
# `unknown`, a union member as a reason to resync, and a status as an error,
# never as a reply to cache.
const version :Text = "2.8.0";

# Conventions for the whole protocol:
#
# - No data is a null pointer, an empty list or text, `unknown` in an enum, or 0
#   where a field says so. The reason is not told apart: not implemented yet,
#   not requested, or not permitted for this caller.
# - In a column that has data, a single row without data holds the type's
#   maximum (0xFFFFFFFF for UInt32, 0xFFFFFFFFFFFFFFFF for UInt64). A column
#   that never has per-row gaps says so, and its maximum is an ordinary value.
# - Cumulative counters go out raw, as the OS keeps them. Rates and deltas are
#   the consumer's, over its own interval.
# - Bytes are bytes, not KiB. Durations and CPU times are in 100 ns units.
# - `sampledAt` is QueryPerformanceCounter converted to 100 ns units: monotonic
#   within one boot, meaningful only as a difference.

interface WindowsAgent {
  # The agent returns `nonce` unchanged. A client puts the request's number
  # there and compares, so a reply that reached the wrong call is caught.
  ping             @0  (meta :Meta.RequestMeta, nonce :UInt64)
                      -> (meta :Meta.ResponseMeta, nonce :UInt64);

  # Conditional: the passports only change when a process starts or exits, or
  # when a service starts or stops, since ProcessInfo.isService is derived from
  # the service inventory. The etag of this answer is the `passportEtag` every
  # other process list refers to.
  getProcesses     @1  (meta :Meta.RequestMeta)
                      -> (meta :Meta.ResponseMeta, processes :List(ProcessInfo));

  # Conditional, with its own etag: moves when any state below changes.
  # `states` covers exactly the pids getProcesses returns under `passportEtag`,
  # which names the latest list with the same membership (see ProcessColumns).
  getProcessStates @2  (meta :Meta.RequestMeta)
                      -> (meta :Meta.ResponseMeta, passportEtag :UInt64, states :List(ProcessState));

  # Conditional: answers notModified while the inventory is unchanged.
  getServices      @3  (meta :Meta.RequestMeta)
                      -> (meta :Meta.ResponseMeta, services :List(ServiceStats));

  # Starts sampling what `spec` asks for, until `sampler` is released. The
  # agent samples each live subscription at its own interval and answers it
  # with its own metrics only; a tick reads only what is due then. A spec with
  # machine groups and no process metrics does not read the process list: its
  # samples carry no rows. The list is read for the specs that ask for a
  # process metric or for no metric at all, and every few seconds when none
  # does.
  subscribe        @4  (meta :Meta.RequestMeta, spec :MetricSpec)
                      -> (meta :Meta.ResponseMeta, sampler :Sampler);

  kill             @5  (meta :Meta.RequestMeta, pid :UInt32) -> (meta :Meta.ResponseMeta, code :UInt32);
  suspend          @6  (meta :Meta.RequestMeta, pid :UInt32) -> (meta :Meta.ResponseMeta, code :UInt32);
  resume           @7  (meta :Meta.RequestMeta, pid :UInt32) -> (meta :Meta.ResponseMeta, code :UInt32);
  setPriority      @8  (meta :Meta.RequestMeta, pid :UInt32, priority :ProcessPriority)
                      -> (meta :Meta.ResponseMeta, code :UInt32);
  setAffinity      @9  (meta :Meta.RequestMeta, pid :UInt32, mask :UInt64)
                      -> (meta :Meta.ResponseMeta, code :UInt32);

  serviceStart     @10 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  serviceStop      @11 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  servicePause     @12 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  serviceResume    @13 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);
  serviceRestart   @14 (meta :Meta.RequestMeta, name :Text) -> (meta :Meta.ResponseMeta, code :UInt32);

  # Calls watcher.changed with the service's status now, then on every change
  # until `handle` is released. While a start, stop, pause or continue is
  # pending, checkpoint and waitHintMs refresh too. watcher.ended is called
  # when following stops: the service was deleted or never existed, or the
  # agent is stopping.
  watchService     @15 (meta :Meta.RequestMeta, name :Text, watcher :ServiceWatcher)
                      -> (meta :Meta.ResponseMeta, handle :WatchHandle);

  # Pushes this subscription's snapshots to `listener` until `handle` is
  # released or the connection goes away; a push replacing the Sampler's long
  # poll. The first update carries the full lists; every later one carries what
  # moved since the update before, and its sample refers to the lists as they
  # stand after that update. One call is in flight at a time: while the
  # listener has not returned, newer samples replace older ones and list
  # changes accumulate, so a slow listener gets fewer calls, never a gap. Paced
  # at spec.intervalMs, like subscribe. An error returned by the listener ends
  # the watch.
  watch            @16 (meta :Meta.RequestMeta, spec :MetricSpec, listener :AgentListener)
                      -> (meta :Meta.ResponseMeta, handle :WatchHandle);

  # Calls listener.events with the process starts and exits the agent still
  # holds, oldest first, then with each new one as it happens, until `handle`
  # is released or the connection goes away. The agent keeps recent events,
  # about the last hour within a few MB; the first batch's historyFrom says
  # exactly where they begin. The held events may come in several batches, each
  # well under a reader's traversal limit. One call is in flight at a time:
  # events that come while the listener has not returned go out together in
  # the next call. An error returned by the listener ends the watch.
  watchProcessEvents @17 (meta :Meta.RequestMeta, listener :ProcessEventListener)
                        -> (meta :Meta.ResponseMeta, handle :WatchHandle);
}

# Implemented by the client and called by the agent, like AgentListener.
interface ProcessEventListener {
  events @0 (meta :Meta.RequestMeta, batch :ProcessEventBatch) -> ();

  # The watch stopped for good: the agent is stopping.
  ended  @1 (meta :Meta.RequestMeta) -> ();
}

struct ProcessEventBatch {
  # In the first batch only, 0 after it: the FILETIME from which on nothing is
  # missing, either the agent's start or the oldest event it still held.
  historyFrom @0 :UInt64;

  # In order of time.
  events      @1 :List(ProcessEvent);

  # Events missing since the previous batch, 0 normally: dropped by the kernel
  # (ETW buffers full) or by the agent, when a listener fell so far behind that
  # events left the agent's hold before they were sent.
  lost        @2 :UInt32;
}

struct ProcessEvent {
  # The instance, as in ProcessInfo.
  pid            @0 :UInt32;
  sequenceNumber @1 :UInt64;

  # The start or the exit, as a FILETIME (100 ns since 1601, UTC).
  time           @2 :UInt64;

  union {
    started @3 :ProcessStarted;
    exited  @4 :ProcessExited;
  }
}

struct ProcessStarted {
  parentPid            @0 :UInt32;

  # 0 when not known (the parent is the System Idle Process).
  parentSequenceNumber @1 :UInt64;

  sessionId            @2 :UInt32;

  # Win32 path; the NT path when no drive letter maps to it.
  imagePath            @3 :Text;

  # As the process was created, unparsed: unlike ProcessInfo.cmdline it keeps
  # the quoting. Empty when the kernel's event did not come.
  commandLine          @4 :Text;

  # DOMAIN\name of the token's user; the SID string when it does not resolve.
  user                 @5 :Text;

  elevated             @6 :Toggle;
  packageFullName      @7 :Text;

  # Read from the process after it started. Empty when it exited first or
  # could not be read: short-lived processes usually have none.
  workingDirectory     @8 :Text;

  # Path of the Task Scheduler task that started it, like
  # \Microsoft\Windows\UpdateOrchestrator\Schedule Scan. Empty otherwise.
  scheduledTask        @9 :Text;

  # Services hosted by the parent when the process started: a process a
  # service launched names that service. Empty otherwise.
  parentServices       @10 :List(Text);
}

# Totals over the process's whole life, as the kernel reports them at exit.
# The names follow ProcessColumns.
struct ProcessExited {
  exitCode     @0 :UInt32;

  # CPU cycles, not time: the kernel reports cycles here.
  cpuCycles    @1 :UInt64;

  # All I/O the process issued, as ioReadOps and its neighbours count it. The
  # kernel counts the bytes in KiB; they go out in bytes, so in steps of 1024.
  ioReadOps    @2 :UInt64;
  ioWriteOps   @3 :UInt64;
  ioReadBytes  @4 :UInt64;
  ioWriteBytes @5 :UInt64;

  peakCommit   @6 :UInt64;

  # Handles open at exit.
  handles      @7 :UInt32;

  hardFaults   @8 :UInt32;
}

# Implemented by the client and called by the agent, like ServiceWatcher.
interface AgentListener {
  update @0 (meta :Meta.RequestMeta, lists :ListsUpdate,
             processes :ProcessColumns, machine :MachineSample) -> ();

  # The watch stopped for good: the agent is stopping.
  ended  @1 (meta :Meta.RequestMeta) -> ();
}

# What changed in the conditional lists since the previous update. Rows are
# keyed by sequenceNumber. The etags are always set and name the lists after
# this update: `passportEtag` is the one `processes` in the same update refers to.
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

  services :union {
    unchanged @8 :Void;
    full      @9 :List(ServiceStats);
  }
  servicesEtag @10 :UInt64;
}

# Applies to the passports under `baseEtag`. A client holding another etag
# resyncs through getProcesses, or releases the watch and watches again. A
# restamp without a membership change is a delta with both lists empty.
struct PassportsDelta {
  baseEtag @0 :UInt64;

  # Sequence numbers of processes that exited. Changes accumulated while the
  # listener was busy may name a process the client never received; ignore it.
  left     @1 :List(UInt64);

  # Processes that started or whose passport changed, as whole rows.
  upserted @2 :List(ProcessInfo);
}

# Applies to the states under `baseEtag`, like PassportsDelta.
struct StatesDelta {
  baseEtag @0 :UInt64;
  left     @1 :List(UInt64);
  upserted @2 :List(ProcessState);
}

# What one subscription wants. To change it, subscribe again and release the
# old sampler.
struct MetricSpec {
  # How often this subscriber wants a fresh snapshot.
  intervalMs @0 :UInt32;
  processes  @1 :List(ProcessMetric);
  machine    @2 :List(MachineMetric);
}

# One subscription. Released by the client to stop; the agent then stops
# sampling it, and does the same when the connection goes away.
interface Sampler {
  # The latest snapshot of this subscription's metrics; the response etag is
  # its snapshot number. A long poll: when ifNoneMatch equals the current
  # snapshot, the answer is held until the next one is taken, so a client that
  # always passes the last etag it got receives every snapshot once, paced by
  # the agent. Dropping the call cancels the wait.
  sample @0 (meta :Meta.RequestMeta)
         -> (meta :Meta.ResponseMeta, processes :ProcessColumns, machine :MachineSample);
}

# One value per column, in ProcessColumns order.
enum ProcessMetric {
  cpuUserTime       @0;
  cpuKernelTime     @1;
  cpuCycles         @2;

  workingSet        @3;
  peakWorkingSet    @4;
  privateWorkingSet @5;
  commit            @6;
  pagedPool         @7;
  nonPagedPool      @8;
  pageFaults        @9;

  handles           @10;
  threads           @11;
  userObjects       @12;
  gdiObjects        @13;

  ioReadOps         @14;
  ioWriteOps        @15;
  ioOtherOps        @16;
  ioReadBytes       @17;
  ioWriteBytes      @18;
  ioOtherBytes      @19;

  diskReadOps       @20;
  diskWriteOps      @21;
  diskFlushOps      @22;
  diskReadBytes     @23;
  diskWriteBytes    @24;
  netRxBytes        @25;
  netTxBytes        @26;

  virtualSize       @27;
  peakVirtualSize   @28;
  peakCommit        @29;
  peakPagedPool     @30;
  peakNonPagedPool  @31;
  hardFaults        @32;
  peakThreads       @33;
  contextSwitches   @34;

  gpuDedicated      @35;
  gpuShared         @36;
  gpuEngines        @37;

  exclusiveMapped   @38;
}

# Process metrics as columns: `pids`, `sequenceNumbers` and every non-null
# column have the same length, and row i of every column belongs to the same
# process. A column is null when it was not requested or the agent has no data
# for it.
struct ProcessColumns {
  snapshot          @0  :UInt64;
  sampledAt         @1  :UInt64;

  # The getProcesses etag these rows were sampled against: the rows are exactly
  # the processes getProcesses returns under it. It names the latest list with
  # the same membership, so a list re-issued without a process starting or
  # exiting (enrichment finishing, a service's pid changing isService) restamps
  # it without a new sample. A different etag on the
  # caller's side means its passports are stale and worth refetching; rows
  # still join safely by sequence number, and a row whose sequence number has
  # no passport yet waits for the next getProcesses.
  passportEtag      @2  :UInt64;
  pids              @3  :List(UInt32);

  # ProcessInfo.sequenceNumber of each row: the join key, never reused within a
  # boot. Always set when `pids` is.
  sequenceNumbers   @4  :List(UInt64);

  # Cumulative, 100 ns.
  cpuUserTime       @5  :List(UInt64);
  cpuKernelTime     @6  :List(UInt64);
  # Cumulative.
  cpuCycles         @7  :List(UInt64);

  # Bytes, current. The shared working set is workingSet - privateWorkingSet.
  workingSet        @8  :List(UInt64);
  peakWorkingSet    @9  :List(UInt64);
  privateWorkingSet @10 :List(UInt64);
  commit            @11 :List(UInt64);
  pagedPool         @12 :List(UInt64);
  nonPagedPool      @13 :List(UInt64);
  # Cumulative, and 32-bit in the kernel: take deltas modulo 2^32. Never has
  # per-row gaps.
  pageFaults        @14 :List(UInt32);

  handles           @15 :List(UInt32);
  threads           @16 :List(UInt32);

  # win32k answers only within the querying session, so rows from a session
  # the agent cannot query hold 0xFFFFFFFF.
  userObjects       @17 :List(UInt32);
  gdiObjects        @18 :List(UInt32);

  # All I/O the process issued (files, devices, pipes), cumulative.
  ioReadOps         @19 :List(UInt64);
  ioWriteOps        @20 :List(UInt64);
  ioOtherOps        @21 :List(UInt64);
  ioReadBytes       @22 :List(UInt64);
  ioWriteBytes      @23 :List(UInt64);
  ioOtherBytes      @24 :List(UInt64);

  # Physical disk and network traffic attributed to the process, cumulative.
  # The network counters start when the agent first saw the process, not at
  # process start; deltas are unaffected.
  diskReadOps       @25 :List(UInt64);
  diskWriteOps      @26 :List(UInt64);
  diskFlushOps      @27 :List(UInt64);
  diskReadBytes     @28 :List(UInt64);
  diskWriteBytes    @29 :List(UInt64);
  netRxBytes        @30 :List(UInt64);
  netTxBytes        @31 :List(UInt64);

  # Bytes of address space, current and peak. Includes reservations, so a
  # 64-bit process shows terabytes.
  virtualSize       @32 :List(UInt64);
  peakVirtualSize   @33 :List(UInt64);

  # Bytes, the peaks of commit, pagedPool and nonPagedPool.
  peakCommit        @34 :List(UInt64);
  peakPagedPool     @35 :List(UInt64);
  peakNonPagedPool  @36 :List(UInt64);

  # Page faults served from disk. Cumulative and 32-bit in the kernel, like
  # pageFaults: take deltas modulo 2^32. Never has per-row gaps.
  hardFaults        @37 :List(UInt32);

  # The most threads the process has had at once.
  peakThreads       @38 :List(UInt32);

  # Over all the process's threads, exited ones included; cumulative.
  contextSwitches   @39 :List(UInt64);

  # Bytes the process has committed on the hardware adapters, summed over
  # them: in the adapters' own memory (dedicated) and in system memory they
  # map (shared); the PDH counters GPU Process Memory Local Usage and Shared
  # Usage. 0 for a process without a GPU context.
  gpuDedicated      @40 :List(UInt64);
  gpuShared         @41 :List(UInt64);

  # Sparse: one entry per process and engine it has run on; processes that
  # never ran on an engine have none. A process the agent could not query has
  # none either, and holds the maximum in gpuDedicated, which tells the two
  # apart. Task Manager's GPU column is the busiest engine's share of wall
  # time over an interval, and its GPU engine column names that engine.
  gpuEngines        @42 :List(ProcessGpuEngine);

  # Bytes of the working set in pages backed by a section (mapped files,
  # images, pagefile-backed shared memory) that no other process has in its
  # working set: QueryWorkingSet pages with Shared and ShareCount 1. Standby
  # and modified pages are not counted. privateWorkingSet + exclusiveMapped is
  # roughly what the process alone holds in memory. Probed as the process
  # shows up, when its shared working set (workingSet - privateWorkingSet)
  # moves by 4 MB or a twentieth, and at least every 5 minutes, so a row can
  # be that much older than sampledAt. A row holds the maximum until its
  # process is first probed and when its working set cannot be read
  # (protected, access denied); every row does when the agent is not
  # elevated, since Windows tells share counts only to an elevated caller.
  exclusiveMapped   @43 :List(UInt64);
}

struct ProcessGpuEngine {
  # Index into pids / sequenceNumbers of the same ProcessColumns.
  row         @0 :UInt32;

  # GpuAdapter.luid and GpuEngine.ordinal in the machine's gpus.
  adapterLuid @1 :UInt64;
  engine      @2 :UInt32;

  # Cumulative, 100 ns, raw as the kernel keeps it: deltas modulo 2^64.
  runningTime @3 :UInt64;
}

# Machine data comes in groups; a group is null when it was not requested or
# the agent has no data for it.
enum MachineMetric {
  cpu     @0;
  memory  @1;
  disk    @2;
  network @3;

  processors @4;
  gpu        @5;
  networkAdapters @6;
}

struct MachineSample {
  snapshot  @0 :UInt64;
  sampledAt @1 :UInt64;
  cpu       @2 :MachineCpu;
  memory    @3 :MachineMemory;
  disk      @4 :MachineDisk;
  network   @5 :MachineNetwork;

  # One entry per logical processor: group 0 first, processor order within a
  # group. Their sums are MachineCpu's times.
  processors @6 :List(MachineProcessor);

  # Render adapters only: the Microsoft Basic Render Driver is left out, and
  # compute-only adapters such as NPUs are not enumerated.
  gpus       @7 :List(GpuAdapter);

  # The adapters that are up, minus loopback, tunnels and the NDIS filter
  # layers stacked on an adapter, which repeat its counters.
  networkAdapters @8 :List(NetworkAdapter);
}

struct NetworkAdapter {
  # NET_LUID; stable while the adapter exists.
  luid              @0 :UInt64;

  # The connection's name as the Network Connections folder shows it
  # ("Ethernet", "Wi-Fi"), and the driver's name for the device.
  name              @1 :Text;
  description       @2 :Text;

  # IANA ifType: 6 is Ethernet, 71 is Wi-Fi, 53 a vendor's virtual adapter.
  ifType            @3 :UInt32;

  # A physical adapter, as opposed to a virtual switch, a VM host-only
  # adapter or a VPN. A sum over all adapters counts forwarded traffic twice:
  # a VM's traffic through vEthernet and again through the physical NIC.
  hardware          @4 :Bool;

  # Bits per second, as the driver reports them now.
  receiveLinkSpeed  @5 :UInt64;
  transmitLinkSpeed @6 :UInt64;

  # Bytes, cumulative, as the adapter counts them: every protocol and header,
  # unlike MachineNetwork's TCP and UDP payload.
  rxBytes           @7 :UInt64;
  txBytes           @8 :UInt64;
}

struct GpuAdapter {
  # HighPart << 32 | LowPart. Stable while the adapter is present; a driver
  # restart can change it, and its counters then start over.
  luid            @0 :UInt64;
  name            @1 :Text;

  # Bytes. Dedicated is the adapter's own memory, shared the system memory it
  # can map; usage is what Task Manager's Performance page shows.
  dedicatedLimit  @2 :UInt64;
  dedicatedUsage  @3 :UInt64;
  sharedLimit     @4 :UInt64;
  sharedUsage     @5 :UInt64;

  # From the driver, 0 when it does not report them. Temperature in tenths of
  # a degree Celsius, power in tenths of a percent of the adapter's maximum,
  # memory frequency in Hz.
  temperature     @6 :UInt32;
  fanRpm          @7 :UInt32;
  power           @8 :UInt32;
  memoryFrequency @9 :UInt64;

  engines         @10 :List(GpuEngine);
}

struct GpuEngine {
  # The node ordinal; ProcessGpuEngine.engine refers to it.
  ordinal      @0 :UInt32;
  type         @1 :GpuEngineType;

  # The driver's name for it, often empty; Task Manager then names it by type
  # ("3D", "Copy", "Video Decode").
  name         @2 :Text;

  # 100 ns: the sum over processes of the time they ran on this engine,
  # counted from when the agent first read it; the kernel's own per-engine
  # counter costs milliseconds per read in the driver. The absolute value
  # means nothing: take deltas modulo 2^64. A process that exits between two
  # reads loses what it ran since the last one. Task Manager's per-engine
  # graph is a per-process sum too.
  runningTime  @3 :UInt64;

  # Not read, always 0: each read costs about a millisecond in the driver.
  frequency    @4 :UInt64;

  # Hz, read once per adapter; 0 when the driver does not report it.
  maxFrequency @5 :UInt64;
}

# DXGK_ENGINE_TYPE, with the same values.
enum GpuEngineType {
  other           @0;
  threeD          @1;
  videoDecode     @2;
  videoEncode     @3;
  videoProcessing @4;
  sceneAssembly   @5;
  copy            @6;
  overlay         @7;
  crypto          @8;
  videoCodec      @9;
}

# Cumulative, 100 ns; kernel time includes idle time, as in MachineCpu.
struct MachineProcessor {
  idleTime      @0 :UInt64;
  kernelTime    @1 :UInt64;
  userTime      @2 :UInt64;
  interruptTime @3 :UInt64;
  dpcTime       @4 :UInt64;
}

# Sums over every logical processor in every processor group. Kernel time
# includes idle time, so busy time is kernel + user - idle, and kernel + user
# is the total CPU time a process's own kernel + user is a share of.
struct MachineCpu {
  # Cumulative, 100 ns.
  idleTime      @0 :UInt64;
  kernelTime    @1 :UInt64;
  userTime      @2 :UInt64;
  interruptTime @3 :UInt64;
  dpcTime       @4 :UInt64;

  maxMhz        @5 :UInt32;
  currentMhz    @6 :UInt32;
}

# Bytes, current.
struct MachineMemory {
  totalPhysical     @0 :UInt64;
  availablePhysical @1 :UInt64;

  # The commit limit (RAM plus page files) and the commit charge now.
  commitLimit       @2 :UInt64;
  committed         @3 :UInt64;
}

# All physical disks together, cumulative.
struct MachineDisk {
  readOps    @0 :UInt64;
  writeOps   @1 :UInt64;
  readBytes  @2 :UInt64;
  writeBytes @3 :UInt64;
}

# TCP and UDP payload over the whole machine, loopback included, counted from
# the kernel's TcpIp and UdpIp events since the agent started: the counters
# start over when the agent restarts, so a delta across a reconnect is not one.
# Per-adapter traffic, headers included, is in MachineSample.networkAdapters.
struct MachineNetwork {
  rxBytes @0 :UInt64;
  txBytes @1 :UInt64;
}

# Implemented by the client and called by the agent. `meta` is there because
# every method in the protocol carries it; the agent sends it empty.
interface ServiceWatcher {
  changed @0 (meta :Meta.RequestMeta, status :ServiceStatus) -> ();
  ended   @1 (meta :Meta.RequestMeta) -> ();
}

# Released by the client to stop watching; it has no methods.
interface WatchHandle {}

struct ServiceStatus {
  state           @0 :ServiceState;

  # 0 while the service has no process.
  pid             @1 :UInt32;

  # Win32 code the service stopped with.
  exitCode        @2 :UInt32;

  # The service's own code, meaningful when exitCode is
  # ERROR_SERVICE_SPECIFIC_ERROR (1066).
  serviceExitCode @3 :UInt32;

  # Grows while a pending step makes progress.
  checkpoint      @4 :UInt32;

  # How long the service expects its pending step to take.
  waitHintMs      @5 :UInt32;
}

enum Toggle {
  unknown @0;
  off     @1;
  on      @2;
}

enum ProcessPriority {
  unknown     @0;
  idle        @1;
  belowNormal @2;
  normal      @3;
  aboveNormal @4;
  high        @5;
  realtime    @6;
}

enum SignatureStatus {
  unknown    @0;
  unsigned   @1;
  microsoft  @2;
  thirdParty @3;
}

# The instruction set the process's code runs as, as Task Manager shows it.
# `arm64X86Compatible` is CHPE and `arm64X64Compatible` is ARM64EC. 16, 32 or
# 64-bit follows from it.
enum Architecture {
  unknown            @0;
  x86                @1;
  x64                @2;
  arm                @3;
  arm64              @4;
  arm64X86Compatible @5;
  arm64X64Compatible @6;
}

enum UacVirtualization {
  unknown    @0;
  notAllowed @1;
  disabled   @2;
  enabled    @3;
}

# Hardware-enforced stack protection (CET shadow stacks; return address
# signing on ARM64): off, compatible modules only, or all modules, each of the
# last two optionally in audit mode.
enum StackProtection {
  unknown         @0;
  off             @1;
  compatible      @2;
  strict          @3;
  compatibleAudit @4;
  strictAudit     @5;
}

enum ExtendedCfg {
  unknown @0;
  off     @1;
  audit   @2;
  on      @3;
}

# `veryLow` is what Task Manager shows as Background.
enum IoPriority {
  unknown  @0;
  veryLow  @1;
  low      @2;
  normal   @3;
  high     @4;
  critical @5;
}

enum DpiAwareness {
  unknown         @0;
  unaware         @1;
  system          @2;
  perMonitor      @3;
  perMonitorV2    @4;
  unawareGdiScaled @5;
}

enum Isolation {
  unknown      @0;
  none         @1;
  appContainer @2;
  uwp          @3;
  silo         @4;
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

  # Creation time as a FILETIME (100 ns since 1601, UTC). 0 when unknown.
  startTime            @14 :UInt64;

  # ProcessSequenceNumber: unique per process within one boot, so unlike the pid
  # it is never reused. 0 only for the System Idle Process (pid 0), which joins
  # by pid.
  sequenceNumber       @15 :UInt64;

  # DOMAIN\name of the token's user.
  user                 @16 :Text;

  # 32 or 64-bit follows from it: x86 is 32-bit.
  architecture         @17 :Architecture;

  elevated             @18 :Toggle;
  uacVirtualization    @19 :UacVirtualization;
  isolation            @20 :Isolation;
  dpiAwareness         @21 :DpiAwareness;

  # Null when the process could not be queried.
  mitigations          @22 :Mitigations;

  # A packaged app's PublisherDisplayName from its manifest, otherwise the
  # signer's subject name.
  publisher            @23 :Text;
}

struct Mitigations {
  dep             @0 :Toggle;
  stackProtection @1 :StackProtection;
  extendedCfg     @2 :ExtendedCfg;
}

# How a process is running right now. Changes rarely, so it travels through
# the conditional getProcessStates.
struct ProcessState {
  pid             @0 :UInt32;
  # ProcessInfo.sequenceNumber: the join key.
  sequenceNumber  @1 :UInt64;

  # Every thread is waiting with reason Suspended.
  suspended       @2 :Toggle;

  # Task Manager's efficiency mode: EcoQoS throttling together with the Idle
  # priority class, as Task Manager sets it. It shows in the Status column.
  efficiencyMode  @3 :Toggle;
  basePriority    @4 :ProcessPriority;

  # EcoQoS throttling on its own (EXECUTION_SPEED in the state mask); the
  # Power throttling column.
  powerThrottling @5 :Toggle;

  # Kernel id of the job object the process belongs to, 0 when it is in none.
  jobObjectId     @6 :UInt32;
  ioPriority      @7 :IoPriority;

  # The process runs a virtual machine on the Windows Hypervisor Platform:
  # it has loaded WinHvPlatform.dll and holds \Device\VidExo open, the
  # handle of a VID partition (VMware Workstation and VirtualBox on Hyper-V,
  # QEMU with WHPX). Its guest RAM is often a mapped file, which the private
  # working set does not count, so workingSet is the better measure of what it
  # holds. Off for a process whose shared working set is under 256 MB, which
  # is not checked; unknown when its modules or handles cannot be read.
  vmHost          @8 :Toggle;
}
