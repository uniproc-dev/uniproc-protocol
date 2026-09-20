@0xd0b00dd26d1a5151;

using Meta = import "meta.capnp";

interface LinuxAgent {
  ping      @0 (meta :Meta.RequestMeta) -> (meta :Meta.ResponseMeta);
  getReport @1 (meta :Meta.RequestMeta) -> (meta :Meta.ResponseMeta, report :Report);
}

struct Report {
  machine          @0 :MachineStats;
  processes        @1 :List(ProcessStats);
  environments     @2 :List(EnvironmentInfo);
  dockerContainers @3 :List(DockerContainerInfo);
}

struct MachineStats {
  totalKb     @0 :UInt64;
  freeKb      @1 :UInt64;
  availableKb @2 :UInt64;
  usedKb      @3 :UInt64;
  cachedKb    @4 :UInt64;

  busyNs      @5 :UInt64;
  lastTsc     @6 :UInt64;

  vsockRxBytes @7 :UInt64;
  vsockTxBytes @8 :UInt64;
  p9RxBytes    @9 :UInt64;
  p9TxBytes    @10 :UInt64;

  tcpTxLoBytes     @11 :UInt64;
  tcpRxLoBytes     @12 :UInt64;
  tcpTxRemoteBytes @13 :UInt64;
  tcpRxRemoteBytes @14 :UInt64;
  udpTxLoBytes     @15 :UInt64;
  udpRxLoBytes     @16 :UInt64;
  udpTxRemoteBytes @17 :UInt64;
  udpRxRemoteBytes @18 :UInt64;
  udsTxBytes       @19 :UInt64;
  udsRxBytes       @20 :UInt64;

  diskReadBytes  @21 :UInt64;
  diskWriteBytes @22 :UInt64;
  diskReadIops   @23 :UInt64;
  diskWriteIops  @24 :UInt64;

  pipeReadBytes  @25 :UInt64;
  pipeWriteBytes @26 :UInt64;
  sendfileBytes  @27 :UInt64;

  cpuCount       @28 :UInt32;
}

struct ProcessStats {
  globalPid @0 :UInt32;
  localPid  @1 :UInt32;
  mntNs     @2 :UInt64;
  pidNs     @3 :UInt64;
  name      @4 :Text;

  cpuPercent   @5 :Float32;
  rssKb        @6 :UInt64;
  lastActiveNs @7 :UInt64;

  vsockRxBytes @8 :UInt64;
  vsockTxBytes @9 :UInt64;
  p9RxBytes    @10 :UInt64;
  p9TxBytes    @11 :UInt64;

  tcpTxLoBytes     @12 :UInt64;
  tcpRxLoBytes     @13 :UInt64;
  tcpTxRemoteBytes @14 :UInt64;
  tcpRxRemoteBytes @15 :UInt64;
  udpTxLoBytes     @16 :UInt64;
  udpRxLoBytes     @17 :UInt64;
  udpTxRemoteBytes @18 :UInt64;
  udpRxRemoteBytes @19 :UInt64;
  udsTxBytes       @20 :UInt64;
  udsRxBytes       @21 :UInt64;

  diskReadBytes  @22 :UInt64;
  diskWriteBytes @23 :UInt64;
  diskReadIops   @24 :UInt64;
  diskWriteIops  @25 :UInt64;

  pipeReadBytes  @26 :UInt64;
  pipeWriteBytes @27 :UInt64;
  sendfileBytes  @28 :UInt64;
}

struct EnvironmentInfo {
  mntNs @0 :UInt64;
  pidNs @1 :UInt64;
  kind  @2 :EnvironmentKind;
  # Distro name for currentDistro, container id for dockerContainer,
  # empty otherwise.
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
  rawJson    @4 :Text;
}
