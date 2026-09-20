@0xa3f1c07d84be5d29;

# Every method in this protocol carries `meta` as its first parameter and, when
# it has results at all, as its first result. A field added here therefore
# appears on every call at once.

struct RequestMeta {
  # Tag the caller already holds, echoed from a previous ResponseMeta.etag.
  # Zero means "nothing cached"; methods that are not conditional ignore it.
  ifNoneMatch @0 :UInt64;
}

struct ResponseMeta {
  # Tag identifying this exact payload. Opaque to the caller: it is only ever
  # echoed back in RequestMeta.ifNoneMatch. Zero means "do not cache".
  etag   @0 :UInt64;
  status @1 :ResponseStatus;
}

enum ResponseStatus {
  ok @0;

  # The caller's ifNoneMatch still matches, so the payload fields of the
  # result are left unset. Only ever sent in reply to a request that carried
  # a non-zero ifNoneMatch.
  notModified @1;
}
