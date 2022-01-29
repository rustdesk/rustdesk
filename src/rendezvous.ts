/* eslint-disable */
import Long from "long";
import _m0 from "protobufjs/minimal";

export const protobufPackage = "hbb";

export enum ConnType {
  DEFAULT_CONN = 0,
  FILE_TRANSFER = 1,
  PORT_FORWARD = 2,
  RDP = 3,
  UNRECOGNIZED = -1,
}

export function connTypeFromJSON(object: any): ConnType {
  switch (object) {
    case 0:
    case "DEFAULT_CONN":
      return ConnType.DEFAULT_CONN;
    case 1:
    case "FILE_TRANSFER":
      return ConnType.FILE_TRANSFER;
    case 2:
    case "PORT_FORWARD":
      return ConnType.PORT_FORWARD;
    case 3:
    case "RDP":
      return ConnType.RDP;
    case -1:
    case "UNRECOGNIZED":
    default:
      return ConnType.UNRECOGNIZED;
  }
}

export function connTypeToJSON(object: ConnType): string {
  switch (object) {
    case ConnType.DEFAULT_CONN:
      return "DEFAULT_CONN";
    case ConnType.FILE_TRANSFER:
      return "FILE_TRANSFER";
    case ConnType.PORT_FORWARD:
      return "PORT_FORWARD";
    case ConnType.RDP:
      return "RDP";
    default:
      return "UNKNOWN";
  }
}

export enum NatType {
  UNKNOWN_NAT = 0,
  ASYMMETRIC = 1,
  SYMMETRIC = 2,
  UNRECOGNIZED = -1,
}

export function natTypeFromJSON(object: any): NatType {
  switch (object) {
    case 0:
    case "UNKNOWN_NAT":
      return NatType.UNKNOWN_NAT;
    case 1:
    case "ASYMMETRIC":
      return NatType.ASYMMETRIC;
    case 2:
    case "SYMMETRIC":
      return NatType.SYMMETRIC;
    case -1:
    case "UNRECOGNIZED":
    default:
      return NatType.UNRECOGNIZED;
  }
}

export function natTypeToJSON(object: NatType): string {
  switch (object) {
    case NatType.UNKNOWN_NAT:
      return "UNKNOWN_NAT";
    case NatType.ASYMMETRIC:
      return "ASYMMETRIC";
    case NatType.SYMMETRIC:
      return "SYMMETRIC";
    default:
      return "UNKNOWN";
  }
}

export interface RegisterPeer {
  id: string;
  serial: number;
}

export interface RegisterPeerResponse {
  request_pk: boolean;
}

export interface PunchHoleRequest {
  id: string;
  nat_type: NatType;
  licence_key: string;
  conn_type: ConnType;
}

export interface PunchHole {
  socket_addr: Uint8Array;
  relay_server: string;
  nat_type: NatType;
}

export interface TestNatRequest {
  serial: number;
}

/** per my test, uint/int has no difference in encoding, int not good for negative, use sint for negative */
export interface TestNatResponse {
  port: number;
  /** for mobile */
  cu: ConfigUpdate | undefined;
}

export interface PunchHoleSent {
  socket_addr: Uint8Array;
  id: string;
  relay_server: string;
  nat_type: NatType;
  version: string;
}

export interface RegisterPk {
  id: string;
  uuid: Uint8Array;
  pk: Uint8Array;
  old_id: string;
}

export interface RegisterPkResponse {
  result: RegisterPkResponse_Result;
}

export enum RegisterPkResponse_Result {
  OK = 0,
  UUID_MISMATCH = 2,
  ID_EXISTS = 3,
  TOO_FREQUENT = 4,
  INVALID_ID_FORMAT = 5,
  NOT_SUPPORT = 6,
  SERVER_ERROR = 7,
  UNRECOGNIZED = -1,
}

export function registerPkResponse_ResultFromJSON(
  object: any
): RegisterPkResponse_Result {
  switch (object) {
    case 0:
    case "OK":
      return RegisterPkResponse_Result.OK;
    case 2:
    case "UUID_MISMATCH":
      return RegisterPkResponse_Result.UUID_MISMATCH;
    case 3:
    case "ID_EXISTS":
      return RegisterPkResponse_Result.ID_EXISTS;
    case 4:
    case "TOO_FREQUENT":
      return RegisterPkResponse_Result.TOO_FREQUENT;
    case 5:
    case "INVALID_ID_FORMAT":
      return RegisterPkResponse_Result.INVALID_ID_FORMAT;
    case 6:
    case "NOT_SUPPORT":
      return RegisterPkResponse_Result.NOT_SUPPORT;
    case 7:
    case "SERVER_ERROR":
      return RegisterPkResponse_Result.SERVER_ERROR;
    case -1:
    case "UNRECOGNIZED":
    default:
      return RegisterPkResponse_Result.UNRECOGNIZED;
  }
}

export function registerPkResponse_ResultToJSON(
  object: RegisterPkResponse_Result
): string {
  switch (object) {
    case RegisterPkResponse_Result.OK:
      return "OK";
    case RegisterPkResponse_Result.UUID_MISMATCH:
      return "UUID_MISMATCH";
    case RegisterPkResponse_Result.ID_EXISTS:
      return "ID_EXISTS";
    case RegisterPkResponse_Result.TOO_FREQUENT:
      return "TOO_FREQUENT";
    case RegisterPkResponse_Result.INVALID_ID_FORMAT:
      return "INVALID_ID_FORMAT";
    case RegisterPkResponse_Result.NOT_SUPPORT:
      return "NOT_SUPPORT";
    case RegisterPkResponse_Result.SERVER_ERROR:
      return "SERVER_ERROR";
    default:
      return "UNKNOWN";
  }
}

export interface PunchHoleResponse {
  socket_addr: Uint8Array;
  pk: Uint8Array;
  failure: PunchHoleResponse_Failure;
  relay_server: string;
  nat_type: NatType | undefined;
  is_local: boolean | undefined;
  other_failure: string;
}

export enum PunchHoleResponse_Failure {
  ID_NOT_EXIST = 0,
  OFFLINE = 2,
  LICENSE_MISMATCH = 3,
  LICENSE_OVERUSE = 4,
  UNRECOGNIZED = -1,
}

export function punchHoleResponse_FailureFromJSON(
  object: any
): PunchHoleResponse_Failure {
  switch (object) {
    case 0:
    case "ID_NOT_EXIST":
      return PunchHoleResponse_Failure.ID_NOT_EXIST;
    case 2:
    case "OFFLINE":
      return PunchHoleResponse_Failure.OFFLINE;
    case 3:
    case "LICENSE_MISMATCH":
      return PunchHoleResponse_Failure.LICENSE_MISMATCH;
    case 4:
    case "LICENSE_OVERUSE":
      return PunchHoleResponse_Failure.LICENSE_OVERUSE;
    case -1:
    case "UNRECOGNIZED":
    default:
      return PunchHoleResponse_Failure.UNRECOGNIZED;
  }
}

export function punchHoleResponse_FailureToJSON(
  object: PunchHoleResponse_Failure
): string {
  switch (object) {
    case PunchHoleResponse_Failure.ID_NOT_EXIST:
      return "ID_NOT_EXIST";
    case PunchHoleResponse_Failure.OFFLINE:
      return "OFFLINE";
    case PunchHoleResponse_Failure.LICENSE_MISMATCH:
      return "LICENSE_MISMATCH";
    case PunchHoleResponse_Failure.LICENSE_OVERUSE:
      return "LICENSE_OVERUSE";
    default:
      return "UNKNOWN";
  }
}

export interface ConfigUpdate {
  serial: number;
  rendezvous_servers: string[];
}

export interface RequestRelay {
  id: string;
  uuid: string;
  socket_addr: Uint8Array;
  relay_server: string;
  secure: boolean;
  licence_key: string;
  conn_type: ConnType;
}

export interface RelayResponse {
  socket_addr: Uint8Array;
  uuid: string;
  relay_server: string;
  id: string | undefined;
  pk: Uint8Array | undefined;
  refuse_reason: string;
  version: string;
}

export interface SoftwareUpdate {
  url: string;
}

/**
 * if in same intranet, punch hole won't work both for udp and tcp,
 * even some router has below connection error if we connect itself,
 *  { kind: Other, error: "could not resolve to any address" },
 * so we request local address to connect.
 */
export interface FetchLocalAddr {
  socket_addr: Uint8Array;
  relay_server: string;
}

export interface LocalAddr {
  socket_addr: Uint8Array;
  local_addr: Uint8Array;
  relay_server: string;
  id: string;
  version: string;
}

export interface PeerDiscovery {
  cmd: string;
  mac: string;
  id: string;
  username: string;
  hostname: string;
  platform: string;
  misc: string;
}

export interface RendezvousMessage {
  register_peer: RegisterPeer | undefined;
  register_peer_response: RegisterPeerResponse | undefined;
  punch_hole_request: PunchHoleRequest | undefined;
  punch_hole: PunchHole | undefined;
  punch_hole_sent: PunchHoleSent | undefined;
  punch_hole_response: PunchHoleResponse | undefined;
  fetch_local_addr: FetchLocalAddr | undefined;
  local_addr: LocalAddr | undefined;
  configure_update: ConfigUpdate | undefined;
  register_pk: RegisterPk | undefined;
  register_pk_response: RegisterPkResponse | undefined;
  software_update: SoftwareUpdate | undefined;
  request_relay: RequestRelay | undefined;
  relay_response: RelayResponse | undefined;
  test_nat_request: TestNatRequest | undefined;
  test_nat_response: TestNatResponse | undefined;
  peer_discovery: PeerDiscovery | undefined;
}

function createBaseRegisterPeer(): RegisterPeer {
  return { id: "", serial: 0 };
}

export const RegisterPeer = {
  encode(
    message: RegisterPeer,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== "") {
      writer.uint32(10).string(message.id);
    }
    if (message.serial !== 0) {
      writer.uint32(16).int32(message.serial);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): RegisterPeer {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseRegisterPeer();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.string();
          break;
        case 2:
          message.serial = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): RegisterPeer {
    return {
      id: isSet(object.id) ? String(object.id) : "",
      serial: isSet(object.serial) ? Number(object.serial) : 0,
    };
  },

  toJSON(message: RegisterPeer): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = message.id);
    message.serial !== undefined && (obj.serial = Math.round(message.serial));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RegisterPeer>, I>>(
    object: I
  ): RegisterPeer {
    const message = createBaseRegisterPeer();
    message.id = object.id ?? "";
    message.serial = object.serial ?? 0;
    return message;
  },
};

function createBaseRegisterPeerResponse(): RegisterPeerResponse {
  return { request_pk: false };
}

export const RegisterPeerResponse = {
  encode(
    message: RegisterPeerResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.request_pk === true) {
      writer.uint32(16).bool(message.request_pk);
    }
    return writer;
  },

  decode(
    input: _m0.Reader | Uint8Array,
    length?: number
  ): RegisterPeerResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseRegisterPeerResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 2:
          message.request_pk = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): RegisterPeerResponse {
    return {
      request_pk: isSet(object.request_pk) ? Boolean(object.request_pk) : false,
    };
  },

  toJSON(message: RegisterPeerResponse): unknown {
    const obj: any = {};
    message.request_pk !== undefined && (obj.request_pk = message.request_pk);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RegisterPeerResponse>, I>>(
    object: I
  ): RegisterPeerResponse {
    const message = createBaseRegisterPeerResponse();
    message.request_pk = object.request_pk ?? false;
    return message;
  },
};

function createBasePunchHoleRequest(): PunchHoleRequest {
  return { id: "", nat_type: 0, licence_key: "", conn_type: 0 };
}

export const PunchHoleRequest = {
  encode(
    message: PunchHoleRequest,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== "") {
      writer.uint32(10).string(message.id);
    }
    if (message.nat_type !== 0) {
      writer.uint32(16).int32(message.nat_type);
    }
    if (message.licence_key !== "") {
      writer.uint32(26).string(message.licence_key);
    }
    if (message.conn_type !== 0) {
      writer.uint32(32).int32(message.conn_type);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): PunchHoleRequest {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePunchHoleRequest();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.string();
          break;
        case 2:
          message.nat_type = reader.int32() as any;
          break;
        case 3:
          message.licence_key = reader.string();
          break;
        case 4:
          message.conn_type = reader.int32() as any;
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): PunchHoleRequest {
    return {
      id: isSet(object.id) ? String(object.id) : "",
      nat_type: isSet(object.nat_type) ? natTypeFromJSON(object.nat_type) : 0,
      licence_key: isSet(object.licence_key) ? String(object.licence_key) : "",
      conn_type: isSet(object.conn_type)
        ? connTypeFromJSON(object.conn_type)
        : 0,
    };
  },

  toJSON(message: PunchHoleRequest): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = message.id);
    message.nat_type !== undefined &&
      (obj.nat_type = natTypeToJSON(message.nat_type));
    message.licence_key !== undefined &&
      (obj.licence_key = message.licence_key);
    message.conn_type !== undefined &&
      (obj.conn_type = connTypeToJSON(message.conn_type));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PunchHoleRequest>, I>>(
    object: I
  ): PunchHoleRequest {
    const message = createBasePunchHoleRequest();
    message.id = object.id ?? "";
    message.nat_type = object.nat_type ?? 0;
    message.licence_key = object.licence_key ?? "";
    message.conn_type = object.conn_type ?? 0;
    return message;
  },
};

function createBasePunchHole(): PunchHole {
  return { socket_addr: new Uint8Array(), relay_server: "", nat_type: 0 };
}

export const PunchHole = {
  encode(
    message: PunchHole,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socket_addr.length !== 0) {
      writer.uint32(10).bytes(message.socket_addr);
    }
    if (message.relay_server !== "") {
      writer.uint32(18).string(message.relay_server);
    }
    if (message.nat_type !== 0) {
      writer.uint32(24).int32(message.nat_type);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): PunchHole {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePunchHole();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.socket_addr = reader.bytes();
          break;
        case 2:
          message.relay_server = reader.string();
          break;
        case 3:
          message.nat_type = reader.int32() as any;
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): PunchHole {
    return {
      socket_addr: isSet(object.socket_addr)
        ? bytesFromBase64(object.socket_addr)
        : new Uint8Array(),
      relay_server: isSet(object.relay_server)
        ? String(object.relay_server)
        : "",
      nat_type: isSet(object.nat_type) ? natTypeFromJSON(object.nat_type) : 0,
    };
  },

  toJSON(message: PunchHole): unknown {
    const obj: any = {};
    message.socket_addr !== undefined &&
      (obj.socket_addr = base64FromBytes(
        message.socket_addr !== undefined
          ? message.socket_addr
          : new Uint8Array()
      ));
    message.relay_server !== undefined &&
      (obj.relay_server = message.relay_server);
    message.nat_type !== undefined &&
      (obj.nat_type = natTypeToJSON(message.nat_type));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PunchHole>, I>>(
    object: I
  ): PunchHole {
    const message = createBasePunchHole();
    message.socket_addr = object.socket_addr ?? new Uint8Array();
    message.relay_server = object.relay_server ?? "";
    message.nat_type = object.nat_type ?? 0;
    return message;
  },
};

function createBaseTestNatRequest(): TestNatRequest {
  return { serial: 0 };
}

export const TestNatRequest = {
  encode(
    message: TestNatRequest,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.serial !== 0) {
      writer.uint32(8).int32(message.serial);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): TestNatRequest {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseTestNatRequest();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.serial = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): TestNatRequest {
    return {
      serial: isSet(object.serial) ? Number(object.serial) : 0,
    };
  },

  toJSON(message: TestNatRequest): unknown {
    const obj: any = {};
    message.serial !== undefined && (obj.serial = Math.round(message.serial));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<TestNatRequest>, I>>(
    object: I
  ): TestNatRequest {
    const message = createBaseTestNatRequest();
    message.serial = object.serial ?? 0;
    return message;
  },
};

function createBaseTestNatResponse(): TestNatResponse {
  return { port: 0, cu: undefined };
}

export const TestNatResponse = {
  encode(
    message: TestNatResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.port !== 0) {
      writer.uint32(8).int32(message.port);
    }
    if (message.cu !== undefined) {
      ConfigUpdate.encode(message.cu, writer.uint32(18).fork()).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): TestNatResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseTestNatResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.port = reader.int32();
          break;
        case 2:
          message.cu = ConfigUpdate.decode(reader, reader.uint32());
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): TestNatResponse {
    return {
      port: isSet(object.port) ? Number(object.port) : 0,
      cu: isSet(object.cu) ? ConfigUpdate.fromJSON(object.cu) : undefined,
    };
  },

  toJSON(message: TestNatResponse): unknown {
    const obj: any = {};
    message.port !== undefined && (obj.port = Math.round(message.port));
    message.cu !== undefined &&
      (obj.cu = message.cu ? ConfigUpdate.toJSON(message.cu) : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<TestNatResponse>, I>>(
    object: I
  ): TestNatResponse {
    const message = createBaseTestNatResponse();
    message.port = object.port ?? 0;
    message.cu =
      object.cu !== undefined && object.cu !== null
        ? ConfigUpdate.fromPartial(object.cu)
        : undefined;
    return message;
  },
};

function createBasePunchHoleSent(): PunchHoleSent {
  return {
    socket_addr: new Uint8Array(),
    id: "",
    relay_server: "",
    nat_type: 0,
    version: "",
  };
}

export const PunchHoleSent = {
  encode(
    message: PunchHoleSent,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socket_addr.length !== 0) {
      writer.uint32(10).bytes(message.socket_addr);
    }
    if (message.id !== "") {
      writer.uint32(18).string(message.id);
    }
    if (message.relay_server !== "") {
      writer.uint32(26).string(message.relay_server);
    }
    if (message.nat_type !== 0) {
      writer.uint32(32).int32(message.nat_type);
    }
    if (message.version !== "") {
      writer.uint32(42).string(message.version);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): PunchHoleSent {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePunchHoleSent();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.socket_addr = reader.bytes();
          break;
        case 2:
          message.id = reader.string();
          break;
        case 3:
          message.relay_server = reader.string();
          break;
        case 4:
          message.nat_type = reader.int32() as any;
          break;
        case 5:
          message.version = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): PunchHoleSent {
    return {
      socket_addr: isSet(object.socket_addr)
        ? bytesFromBase64(object.socket_addr)
        : new Uint8Array(),
      id: isSet(object.id) ? String(object.id) : "",
      relay_server: isSet(object.relay_server)
        ? String(object.relay_server)
        : "",
      nat_type: isSet(object.nat_type) ? natTypeFromJSON(object.nat_type) : 0,
      version: isSet(object.version) ? String(object.version) : "",
    };
  },

  toJSON(message: PunchHoleSent): unknown {
    const obj: any = {};
    message.socket_addr !== undefined &&
      (obj.socket_addr = base64FromBytes(
        message.socket_addr !== undefined
          ? message.socket_addr
          : new Uint8Array()
      ));
    message.id !== undefined && (obj.id = message.id);
    message.relay_server !== undefined &&
      (obj.relay_server = message.relay_server);
    message.nat_type !== undefined &&
      (obj.nat_type = natTypeToJSON(message.nat_type));
    message.version !== undefined && (obj.version = message.version);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PunchHoleSent>, I>>(
    object: I
  ): PunchHoleSent {
    const message = createBasePunchHoleSent();
    message.socket_addr = object.socket_addr ?? new Uint8Array();
    message.id = object.id ?? "";
    message.relay_server = object.relay_server ?? "";
    message.nat_type = object.nat_type ?? 0;
    message.version = object.version ?? "";
    return message;
  },
};

function createBaseRegisterPk(): RegisterPk {
  return { id: "", uuid: new Uint8Array(), pk: new Uint8Array(), old_id: "" };
}

export const RegisterPk = {
  encode(
    message: RegisterPk,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== "") {
      writer.uint32(10).string(message.id);
    }
    if (message.uuid.length !== 0) {
      writer.uint32(18).bytes(message.uuid);
    }
    if (message.pk.length !== 0) {
      writer.uint32(26).bytes(message.pk);
    }
    if (message.old_id !== "") {
      writer.uint32(34).string(message.old_id);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): RegisterPk {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseRegisterPk();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.string();
          break;
        case 2:
          message.uuid = reader.bytes();
          break;
        case 3:
          message.pk = reader.bytes();
          break;
        case 4:
          message.old_id = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): RegisterPk {
    return {
      id: isSet(object.id) ? String(object.id) : "",
      uuid: isSet(object.uuid)
        ? bytesFromBase64(object.uuid)
        : new Uint8Array(),
      pk: isSet(object.pk) ? bytesFromBase64(object.pk) : new Uint8Array(),
      old_id: isSet(object.old_id) ? String(object.old_id) : "",
    };
  },

  toJSON(message: RegisterPk): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = message.id);
    message.uuid !== undefined &&
      (obj.uuid = base64FromBytes(
        message.uuid !== undefined ? message.uuid : new Uint8Array()
      ));
    message.pk !== undefined &&
      (obj.pk = base64FromBytes(
        message.pk !== undefined ? message.pk : new Uint8Array()
      ));
    message.old_id !== undefined && (obj.old_id = message.old_id);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RegisterPk>, I>>(
    object: I
  ): RegisterPk {
    const message = createBaseRegisterPk();
    message.id = object.id ?? "";
    message.uuid = object.uuid ?? new Uint8Array();
    message.pk = object.pk ?? new Uint8Array();
    message.old_id = object.old_id ?? "";
    return message;
  },
};

function createBaseRegisterPkResponse(): RegisterPkResponse {
  return { result: 0 };
}

export const RegisterPkResponse = {
  encode(
    message: RegisterPkResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.result !== 0) {
      writer.uint32(8).int32(message.result);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): RegisterPkResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseRegisterPkResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.result = reader.int32() as any;
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): RegisterPkResponse {
    return {
      result: isSet(object.result)
        ? registerPkResponse_ResultFromJSON(object.result)
        : 0,
    };
  },

  toJSON(message: RegisterPkResponse): unknown {
    const obj: any = {};
    message.result !== undefined &&
      (obj.result = registerPkResponse_ResultToJSON(message.result));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RegisterPkResponse>, I>>(
    object: I
  ): RegisterPkResponse {
    const message = createBaseRegisterPkResponse();
    message.result = object.result ?? 0;
    return message;
  },
};

function createBasePunchHoleResponse(): PunchHoleResponse {
  return {
    socket_addr: new Uint8Array(),
    pk: new Uint8Array(),
    failure: 0,
    relay_server: "",
    nat_type: undefined,
    is_local: undefined,
    other_failure: "",
  };
}

export const PunchHoleResponse = {
  encode(
    message: PunchHoleResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socket_addr.length !== 0) {
      writer.uint32(10).bytes(message.socket_addr);
    }
    if (message.pk.length !== 0) {
      writer.uint32(18).bytes(message.pk);
    }
    if (message.failure !== 0) {
      writer.uint32(24).int32(message.failure);
    }
    if (message.relay_server !== "") {
      writer.uint32(34).string(message.relay_server);
    }
    if (message.nat_type !== undefined) {
      writer.uint32(40).int32(message.nat_type);
    }
    if (message.is_local !== undefined) {
      writer.uint32(48).bool(message.is_local);
    }
    if (message.other_failure !== "") {
      writer.uint32(58).string(message.other_failure);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): PunchHoleResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePunchHoleResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.socket_addr = reader.bytes();
          break;
        case 2:
          message.pk = reader.bytes();
          break;
        case 3:
          message.failure = reader.int32() as any;
          break;
        case 4:
          message.relay_server = reader.string();
          break;
        case 5:
          message.nat_type = reader.int32() as any;
          break;
        case 6:
          message.is_local = reader.bool();
          break;
        case 7:
          message.other_failure = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): PunchHoleResponse {
    return {
      socket_addr: isSet(object.socket_addr)
        ? bytesFromBase64(object.socket_addr)
        : new Uint8Array(),
      pk: isSet(object.pk) ? bytesFromBase64(object.pk) : new Uint8Array(),
      failure: isSet(object.failure)
        ? punchHoleResponse_FailureFromJSON(object.failure)
        : 0,
      relay_server: isSet(object.relay_server)
        ? String(object.relay_server)
        : "",
      nat_type: isSet(object.nat_type)
        ? natTypeFromJSON(object.nat_type)
        : undefined,
      is_local: isSet(object.is_local) ? Boolean(object.is_local) : undefined,
      other_failure: isSet(object.other_failure)
        ? String(object.other_failure)
        : "",
    };
  },

  toJSON(message: PunchHoleResponse): unknown {
    const obj: any = {};
    message.socket_addr !== undefined &&
      (obj.socket_addr = base64FromBytes(
        message.socket_addr !== undefined
          ? message.socket_addr
          : new Uint8Array()
      ));
    message.pk !== undefined &&
      (obj.pk = base64FromBytes(
        message.pk !== undefined ? message.pk : new Uint8Array()
      ));
    message.failure !== undefined &&
      (obj.failure = punchHoleResponse_FailureToJSON(message.failure));
    message.relay_server !== undefined &&
      (obj.relay_server = message.relay_server);
    message.nat_type !== undefined &&
      (obj.nat_type =
        message.nat_type !== undefined
          ? natTypeToJSON(message.nat_type)
          : undefined);
    message.is_local !== undefined && (obj.is_local = message.is_local);
    message.other_failure !== undefined &&
      (obj.other_failure = message.other_failure);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PunchHoleResponse>, I>>(
    object: I
  ): PunchHoleResponse {
    const message = createBasePunchHoleResponse();
    message.socket_addr = object.socket_addr ?? new Uint8Array();
    message.pk = object.pk ?? new Uint8Array();
    message.failure = object.failure ?? 0;
    message.relay_server = object.relay_server ?? "";
    message.nat_type = object.nat_type ?? undefined;
    message.is_local = object.is_local ?? undefined;
    message.other_failure = object.other_failure ?? "";
    return message;
  },
};

function createBaseConfigUpdate(): ConfigUpdate {
  return { serial: 0, rendezvous_servers: [] };
}

export const ConfigUpdate = {
  encode(
    message: ConfigUpdate,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.serial !== 0) {
      writer.uint32(8).int32(message.serial);
    }
    for (const v of message.rendezvous_servers) {
      writer.uint32(18).string(v!);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): ConfigUpdate {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseConfigUpdate();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.serial = reader.int32();
          break;
        case 2:
          message.rendezvous_servers.push(reader.string());
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): ConfigUpdate {
    return {
      serial: isSet(object.serial) ? Number(object.serial) : 0,
      rendezvous_servers: Array.isArray(object?.rendezvous_servers)
        ? object.rendezvous_servers.map((e: any) => String(e))
        : [],
    };
  },

  toJSON(message: ConfigUpdate): unknown {
    const obj: any = {};
    message.serial !== undefined && (obj.serial = Math.round(message.serial));
    if (message.rendezvous_servers) {
      obj.rendezvous_servers = message.rendezvous_servers.map((e) => e);
    } else {
      obj.rendezvous_servers = [];
    }
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<ConfigUpdate>, I>>(
    object: I
  ): ConfigUpdate {
    const message = createBaseConfigUpdate();
    message.serial = object.serial ?? 0;
    message.rendezvous_servers = object.rendezvous_servers?.map((e) => e) || [];
    return message;
  },
};

function createBaseRequestRelay(): RequestRelay {
  return {
    id: "",
    uuid: "",
    socket_addr: new Uint8Array(),
    relay_server: "",
    secure: false,
    licence_key: "",
    conn_type: 0,
  };
}

export const RequestRelay = {
  encode(
    message: RequestRelay,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== "") {
      writer.uint32(10).string(message.id);
    }
    if (message.uuid !== "") {
      writer.uint32(18).string(message.uuid);
    }
    if (message.socket_addr.length !== 0) {
      writer.uint32(26).bytes(message.socket_addr);
    }
    if (message.relay_server !== "") {
      writer.uint32(34).string(message.relay_server);
    }
    if (message.secure === true) {
      writer.uint32(40).bool(message.secure);
    }
    if (message.licence_key !== "") {
      writer.uint32(50).string(message.licence_key);
    }
    if (message.conn_type !== 0) {
      writer.uint32(56).int32(message.conn_type);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): RequestRelay {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseRequestRelay();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.string();
          break;
        case 2:
          message.uuid = reader.string();
          break;
        case 3:
          message.socket_addr = reader.bytes();
          break;
        case 4:
          message.relay_server = reader.string();
          break;
        case 5:
          message.secure = reader.bool();
          break;
        case 6:
          message.licence_key = reader.string();
          break;
        case 7:
          message.conn_type = reader.int32() as any;
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): RequestRelay {
    return {
      id: isSet(object.id) ? String(object.id) : "",
      uuid: isSet(object.uuid) ? String(object.uuid) : "",
      socket_addr: isSet(object.socket_addr)
        ? bytesFromBase64(object.socket_addr)
        : new Uint8Array(),
      relay_server: isSet(object.relay_server)
        ? String(object.relay_server)
        : "",
      secure: isSet(object.secure) ? Boolean(object.secure) : false,
      licence_key: isSet(object.licence_key) ? String(object.licence_key) : "",
      conn_type: isSet(object.conn_type)
        ? connTypeFromJSON(object.conn_type)
        : 0,
    };
  },

  toJSON(message: RequestRelay): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = message.id);
    message.uuid !== undefined && (obj.uuid = message.uuid);
    message.socket_addr !== undefined &&
      (obj.socket_addr = base64FromBytes(
        message.socket_addr !== undefined
          ? message.socket_addr
          : new Uint8Array()
      ));
    message.relay_server !== undefined &&
      (obj.relay_server = message.relay_server);
    message.secure !== undefined && (obj.secure = message.secure);
    message.licence_key !== undefined &&
      (obj.licence_key = message.licence_key);
    message.conn_type !== undefined &&
      (obj.conn_type = connTypeToJSON(message.conn_type));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RequestRelay>, I>>(
    object: I
  ): RequestRelay {
    const message = createBaseRequestRelay();
    message.id = object.id ?? "";
    message.uuid = object.uuid ?? "";
    message.socket_addr = object.socket_addr ?? new Uint8Array();
    message.relay_server = object.relay_server ?? "";
    message.secure = object.secure ?? false;
    message.licence_key = object.licence_key ?? "";
    message.conn_type = object.conn_type ?? 0;
    return message;
  },
};

function createBaseRelayResponse(): RelayResponse {
  return {
    socket_addr: new Uint8Array(),
    uuid: "",
    relay_server: "",
    id: undefined,
    pk: undefined,
    refuse_reason: "",
    version: "",
  };
}

export const RelayResponse = {
  encode(
    message: RelayResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socket_addr.length !== 0) {
      writer.uint32(10).bytes(message.socket_addr);
    }
    if (message.uuid !== "") {
      writer.uint32(18).string(message.uuid);
    }
    if (message.relay_server !== "") {
      writer.uint32(26).string(message.relay_server);
    }
    if (message.id !== undefined) {
      writer.uint32(34).string(message.id);
    }
    if (message.pk !== undefined) {
      writer.uint32(42).bytes(message.pk);
    }
    if (message.refuse_reason !== "") {
      writer.uint32(50).string(message.refuse_reason);
    }
    if (message.version !== "") {
      writer.uint32(58).string(message.version);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): RelayResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseRelayResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.socket_addr = reader.bytes();
          break;
        case 2:
          message.uuid = reader.string();
          break;
        case 3:
          message.relay_server = reader.string();
          break;
        case 4:
          message.id = reader.string();
          break;
        case 5:
          message.pk = reader.bytes();
          break;
        case 6:
          message.refuse_reason = reader.string();
          break;
        case 7:
          message.version = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): RelayResponse {
    return {
      socket_addr: isSet(object.socket_addr)
        ? bytesFromBase64(object.socket_addr)
        : new Uint8Array(),
      uuid: isSet(object.uuid) ? String(object.uuid) : "",
      relay_server: isSet(object.relay_server)
        ? String(object.relay_server)
        : "",
      id: isSet(object.id) ? String(object.id) : undefined,
      pk: isSet(object.pk) ? bytesFromBase64(object.pk) : undefined,
      refuse_reason: isSet(object.refuse_reason)
        ? String(object.refuse_reason)
        : "",
      version: isSet(object.version) ? String(object.version) : "",
    };
  },

  toJSON(message: RelayResponse): unknown {
    const obj: any = {};
    message.socket_addr !== undefined &&
      (obj.socket_addr = base64FromBytes(
        message.socket_addr !== undefined
          ? message.socket_addr
          : new Uint8Array()
      ));
    message.uuid !== undefined && (obj.uuid = message.uuid);
    message.relay_server !== undefined &&
      (obj.relay_server = message.relay_server);
    message.id !== undefined && (obj.id = message.id);
    message.pk !== undefined &&
      (obj.pk =
        message.pk !== undefined ? base64FromBytes(message.pk) : undefined);
    message.refuse_reason !== undefined &&
      (obj.refuse_reason = message.refuse_reason);
    message.version !== undefined && (obj.version = message.version);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RelayResponse>, I>>(
    object: I
  ): RelayResponse {
    const message = createBaseRelayResponse();
    message.socket_addr = object.socket_addr ?? new Uint8Array();
    message.uuid = object.uuid ?? "";
    message.relay_server = object.relay_server ?? "";
    message.id = object.id ?? undefined;
    message.pk = object.pk ?? undefined;
    message.refuse_reason = object.refuse_reason ?? "";
    message.version = object.version ?? "";
    return message;
  },
};

function createBaseSoftwareUpdate(): SoftwareUpdate {
  return { url: "" };
}

export const SoftwareUpdate = {
  encode(
    message: SoftwareUpdate,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.url !== "") {
      writer.uint32(10).string(message.url);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): SoftwareUpdate {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseSoftwareUpdate();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.url = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): SoftwareUpdate {
    return {
      url: isSet(object.url) ? String(object.url) : "",
    };
  },

  toJSON(message: SoftwareUpdate): unknown {
    const obj: any = {};
    message.url !== undefined && (obj.url = message.url);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<SoftwareUpdate>, I>>(
    object: I
  ): SoftwareUpdate {
    const message = createBaseSoftwareUpdate();
    message.url = object.url ?? "";
    return message;
  },
};

function createBaseFetchLocalAddr(): FetchLocalAddr {
  return { socket_addr: new Uint8Array(), relay_server: "" };
}

export const FetchLocalAddr = {
  encode(
    message: FetchLocalAddr,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socket_addr.length !== 0) {
      writer.uint32(10).bytes(message.socket_addr);
    }
    if (message.relay_server !== "") {
      writer.uint32(18).string(message.relay_server);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FetchLocalAddr {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFetchLocalAddr();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.socket_addr = reader.bytes();
          break;
        case 2:
          message.relay_server = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FetchLocalAddr {
    return {
      socket_addr: isSet(object.socket_addr)
        ? bytesFromBase64(object.socket_addr)
        : new Uint8Array(),
      relay_server: isSet(object.relay_server)
        ? String(object.relay_server)
        : "",
    };
  },

  toJSON(message: FetchLocalAddr): unknown {
    const obj: any = {};
    message.socket_addr !== undefined &&
      (obj.socket_addr = base64FromBytes(
        message.socket_addr !== undefined
          ? message.socket_addr
          : new Uint8Array()
      ));
    message.relay_server !== undefined &&
      (obj.relay_server = message.relay_server);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FetchLocalAddr>, I>>(
    object: I
  ): FetchLocalAddr {
    const message = createBaseFetchLocalAddr();
    message.socket_addr = object.socket_addr ?? new Uint8Array();
    message.relay_server = object.relay_server ?? "";
    return message;
  },
};

function createBaseLocalAddr(): LocalAddr {
  return {
    socket_addr: new Uint8Array(),
    local_addr: new Uint8Array(),
    relay_server: "",
    id: "",
    version: "",
  };
}

export const LocalAddr = {
  encode(
    message: LocalAddr,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socket_addr.length !== 0) {
      writer.uint32(10).bytes(message.socket_addr);
    }
    if (message.local_addr.length !== 0) {
      writer.uint32(18).bytes(message.local_addr);
    }
    if (message.relay_server !== "") {
      writer.uint32(26).string(message.relay_server);
    }
    if (message.id !== "") {
      writer.uint32(34).string(message.id);
    }
    if (message.version !== "") {
      writer.uint32(42).string(message.version);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): LocalAddr {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseLocalAddr();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.socket_addr = reader.bytes();
          break;
        case 2:
          message.local_addr = reader.bytes();
          break;
        case 3:
          message.relay_server = reader.string();
          break;
        case 4:
          message.id = reader.string();
          break;
        case 5:
          message.version = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): LocalAddr {
    return {
      socket_addr: isSet(object.socket_addr)
        ? bytesFromBase64(object.socket_addr)
        : new Uint8Array(),
      local_addr: isSet(object.local_addr)
        ? bytesFromBase64(object.local_addr)
        : new Uint8Array(),
      relay_server: isSet(object.relay_server)
        ? String(object.relay_server)
        : "",
      id: isSet(object.id) ? String(object.id) : "",
      version: isSet(object.version) ? String(object.version) : "",
    };
  },

  toJSON(message: LocalAddr): unknown {
    const obj: any = {};
    message.socket_addr !== undefined &&
      (obj.socket_addr = base64FromBytes(
        message.socket_addr !== undefined
          ? message.socket_addr
          : new Uint8Array()
      ));
    message.local_addr !== undefined &&
      (obj.local_addr = base64FromBytes(
        message.local_addr !== undefined ? message.local_addr : new Uint8Array()
      ));
    message.relay_server !== undefined &&
      (obj.relay_server = message.relay_server);
    message.id !== undefined && (obj.id = message.id);
    message.version !== undefined && (obj.version = message.version);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<LocalAddr>, I>>(
    object: I
  ): LocalAddr {
    const message = createBaseLocalAddr();
    message.socket_addr = object.socket_addr ?? new Uint8Array();
    message.local_addr = object.local_addr ?? new Uint8Array();
    message.relay_server = object.relay_server ?? "";
    message.id = object.id ?? "";
    message.version = object.version ?? "";
    return message;
  },
};

function createBasePeerDiscovery(): PeerDiscovery {
  return {
    cmd: "",
    mac: "",
    id: "",
    username: "",
    hostname: "",
    platform: "",
    misc: "",
  };
}

export const PeerDiscovery = {
  encode(
    message: PeerDiscovery,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.cmd !== "") {
      writer.uint32(10).string(message.cmd);
    }
    if (message.mac !== "") {
      writer.uint32(18).string(message.mac);
    }
    if (message.id !== "") {
      writer.uint32(26).string(message.id);
    }
    if (message.username !== "") {
      writer.uint32(34).string(message.username);
    }
    if (message.hostname !== "") {
      writer.uint32(42).string(message.hostname);
    }
    if (message.platform !== "") {
      writer.uint32(50).string(message.platform);
    }
    if (message.misc !== "") {
      writer.uint32(58).string(message.misc);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): PeerDiscovery {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePeerDiscovery();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.cmd = reader.string();
          break;
        case 2:
          message.mac = reader.string();
          break;
        case 3:
          message.id = reader.string();
          break;
        case 4:
          message.username = reader.string();
          break;
        case 5:
          message.hostname = reader.string();
          break;
        case 6:
          message.platform = reader.string();
          break;
        case 7:
          message.misc = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): PeerDiscovery {
    return {
      cmd: isSet(object.cmd) ? String(object.cmd) : "",
      mac: isSet(object.mac) ? String(object.mac) : "",
      id: isSet(object.id) ? String(object.id) : "",
      username: isSet(object.username) ? String(object.username) : "",
      hostname: isSet(object.hostname) ? String(object.hostname) : "",
      platform: isSet(object.platform) ? String(object.platform) : "",
      misc: isSet(object.misc) ? String(object.misc) : "",
    };
  },

  toJSON(message: PeerDiscovery): unknown {
    const obj: any = {};
    message.cmd !== undefined && (obj.cmd = message.cmd);
    message.mac !== undefined && (obj.mac = message.mac);
    message.id !== undefined && (obj.id = message.id);
    message.username !== undefined && (obj.username = message.username);
    message.hostname !== undefined && (obj.hostname = message.hostname);
    message.platform !== undefined && (obj.platform = message.platform);
    message.misc !== undefined && (obj.misc = message.misc);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PeerDiscovery>, I>>(
    object: I
  ): PeerDiscovery {
    const message = createBasePeerDiscovery();
    message.cmd = object.cmd ?? "";
    message.mac = object.mac ?? "";
    message.id = object.id ?? "";
    message.username = object.username ?? "";
    message.hostname = object.hostname ?? "";
    message.platform = object.platform ?? "";
    message.misc = object.misc ?? "";
    return message;
  },
};

function createBaseRendezvousMessage(): RendezvousMessage {
  return {
    register_peer: undefined,
    register_peer_response: undefined,
    punch_hole_request: undefined,
    punch_hole: undefined,
    punch_hole_sent: undefined,
    punch_hole_response: undefined,
    fetch_local_addr: undefined,
    local_addr: undefined,
    configure_update: undefined,
    register_pk: undefined,
    register_pk_response: undefined,
    software_update: undefined,
    request_relay: undefined,
    relay_response: undefined,
    test_nat_request: undefined,
    test_nat_response: undefined,
    peer_discovery: undefined,
  };
}

export const RendezvousMessage = {
  encode(
    message: RendezvousMessage,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.register_peer !== undefined) {
      RegisterPeer.encode(
        message.register_peer,
        writer.uint32(50).fork()
      ).ldelim();
    }
    if (message.register_peer_response !== undefined) {
      RegisterPeerResponse.encode(
        message.register_peer_response,
        writer.uint32(58).fork()
      ).ldelim();
    }
    if (message.punch_hole_request !== undefined) {
      PunchHoleRequest.encode(
        message.punch_hole_request,
        writer.uint32(66).fork()
      ).ldelim();
    }
    if (message.punch_hole !== undefined) {
      PunchHole.encode(message.punch_hole, writer.uint32(74).fork()).ldelim();
    }
    if (message.punch_hole_sent !== undefined) {
      PunchHoleSent.encode(
        message.punch_hole_sent,
        writer.uint32(82).fork()
      ).ldelim();
    }
    if (message.punch_hole_response !== undefined) {
      PunchHoleResponse.encode(
        message.punch_hole_response,
        writer.uint32(90).fork()
      ).ldelim();
    }
    if (message.fetch_local_addr !== undefined) {
      FetchLocalAddr.encode(
        message.fetch_local_addr,
        writer.uint32(98).fork()
      ).ldelim();
    }
    if (message.local_addr !== undefined) {
      LocalAddr.encode(message.local_addr, writer.uint32(106).fork()).ldelim();
    }
    if (message.configure_update !== undefined) {
      ConfigUpdate.encode(
        message.configure_update,
        writer.uint32(114).fork()
      ).ldelim();
    }
    if (message.register_pk !== undefined) {
      RegisterPk.encode(
        message.register_pk,
        writer.uint32(122).fork()
      ).ldelim();
    }
    if (message.register_pk_response !== undefined) {
      RegisterPkResponse.encode(
        message.register_pk_response,
        writer.uint32(130).fork()
      ).ldelim();
    }
    if (message.software_update !== undefined) {
      SoftwareUpdate.encode(
        message.software_update,
        writer.uint32(138).fork()
      ).ldelim();
    }
    if (message.request_relay !== undefined) {
      RequestRelay.encode(
        message.request_relay,
        writer.uint32(146).fork()
      ).ldelim();
    }
    if (message.relay_response !== undefined) {
      RelayResponse.encode(
        message.relay_response,
        writer.uint32(154).fork()
      ).ldelim();
    }
    if (message.test_nat_request !== undefined) {
      TestNatRequest.encode(
        message.test_nat_request,
        writer.uint32(162).fork()
      ).ldelim();
    }
    if (message.test_nat_response !== undefined) {
      TestNatResponse.encode(
        message.test_nat_response,
        writer.uint32(170).fork()
      ).ldelim();
    }
    if (message.peer_discovery !== undefined) {
      PeerDiscovery.encode(
        message.peer_discovery,
        writer.uint32(178).fork()
      ).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): RendezvousMessage {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseRendezvousMessage();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 6:
          message.register_peer = RegisterPeer.decode(reader, reader.uint32());
          break;
        case 7:
          message.register_peer_response = RegisterPeerResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 8:
          message.punch_hole_request = PunchHoleRequest.decode(
            reader,
            reader.uint32()
          );
          break;
        case 9:
          message.punch_hole = PunchHole.decode(reader, reader.uint32());
          break;
        case 10:
          message.punch_hole_sent = PunchHoleSent.decode(
            reader,
            reader.uint32()
          );
          break;
        case 11:
          message.punch_hole_response = PunchHoleResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 12:
          message.fetch_local_addr = FetchLocalAddr.decode(
            reader,
            reader.uint32()
          );
          break;
        case 13:
          message.local_addr = LocalAddr.decode(reader, reader.uint32());
          break;
        case 14:
          message.configure_update = ConfigUpdate.decode(
            reader,
            reader.uint32()
          );
          break;
        case 15:
          message.register_pk = RegisterPk.decode(reader, reader.uint32());
          break;
        case 16:
          message.register_pk_response = RegisterPkResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 17:
          message.software_update = SoftwareUpdate.decode(
            reader,
            reader.uint32()
          );
          break;
        case 18:
          message.request_relay = RequestRelay.decode(reader, reader.uint32());
          break;
        case 19:
          message.relay_response = RelayResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 20:
          message.test_nat_request = TestNatRequest.decode(
            reader,
            reader.uint32()
          );
          break;
        case 21:
          message.test_nat_response = TestNatResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 22:
          message.peer_discovery = PeerDiscovery.decode(
            reader,
            reader.uint32()
          );
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): RendezvousMessage {
    return {
      register_peer: isSet(object.register_peer)
        ? RegisterPeer.fromJSON(object.register_peer)
        : undefined,
      register_peer_response: isSet(object.register_peer_response)
        ? RegisterPeerResponse.fromJSON(object.register_peer_response)
        : undefined,
      punch_hole_request: isSet(object.punch_hole_request)
        ? PunchHoleRequest.fromJSON(object.punch_hole_request)
        : undefined,
      punch_hole: isSet(object.punch_hole)
        ? PunchHole.fromJSON(object.punch_hole)
        : undefined,
      punch_hole_sent: isSet(object.punch_hole_sent)
        ? PunchHoleSent.fromJSON(object.punch_hole_sent)
        : undefined,
      punch_hole_response: isSet(object.punch_hole_response)
        ? PunchHoleResponse.fromJSON(object.punch_hole_response)
        : undefined,
      fetch_local_addr: isSet(object.fetch_local_addr)
        ? FetchLocalAddr.fromJSON(object.fetch_local_addr)
        : undefined,
      local_addr: isSet(object.local_addr)
        ? LocalAddr.fromJSON(object.local_addr)
        : undefined,
      configure_update: isSet(object.configure_update)
        ? ConfigUpdate.fromJSON(object.configure_update)
        : undefined,
      register_pk: isSet(object.register_pk)
        ? RegisterPk.fromJSON(object.register_pk)
        : undefined,
      register_pk_response: isSet(object.register_pk_response)
        ? RegisterPkResponse.fromJSON(object.register_pk_response)
        : undefined,
      software_update: isSet(object.software_update)
        ? SoftwareUpdate.fromJSON(object.software_update)
        : undefined,
      request_relay: isSet(object.request_relay)
        ? RequestRelay.fromJSON(object.request_relay)
        : undefined,
      relay_response: isSet(object.relay_response)
        ? RelayResponse.fromJSON(object.relay_response)
        : undefined,
      test_nat_request: isSet(object.test_nat_request)
        ? TestNatRequest.fromJSON(object.test_nat_request)
        : undefined,
      test_nat_response: isSet(object.test_nat_response)
        ? TestNatResponse.fromJSON(object.test_nat_response)
        : undefined,
      peer_discovery: isSet(object.peer_discovery)
        ? PeerDiscovery.fromJSON(object.peer_discovery)
        : undefined,
    };
  },

  toJSON(message: RendezvousMessage): unknown {
    const obj: any = {};
    message.register_peer !== undefined &&
      (obj.register_peer = message.register_peer
        ? RegisterPeer.toJSON(message.register_peer)
        : undefined);
    message.register_peer_response !== undefined &&
      (obj.register_peer_response = message.register_peer_response
        ? RegisterPeerResponse.toJSON(message.register_peer_response)
        : undefined);
    message.punch_hole_request !== undefined &&
      (obj.punch_hole_request = message.punch_hole_request
        ? PunchHoleRequest.toJSON(message.punch_hole_request)
        : undefined);
    message.punch_hole !== undefined &&
      (obj.punch_hole = message.punch_hole
        ? PunchHole.toJSON(message.punch_hole)
        : undefined);
    message.punch_hole_sent !== undefined &&
      (obj.punch_hole_sent = message.punch_hole_sent
        ? PunchHoleSent.toJSON(message.punch_hole_sent)
        : undefined);
    message.punch_hole_response !== undefined &&
      (obj.punch_hole_response = message.punch_hole_response
        ? PunchHoleResponse.toJSON(message.punch_hole_response)
        : undefined);
    message.fetch_local_addr !== undefined &&
      (obj.fetch_local_addr = message.fetch_local_addr
        ? FetchLocalAddr.toJSON(message.fetch_local_addr)
        : undefined);
    message.local_addr !== undefined &&
      (obj.local_addr = message.local_addr
        ? LocalAddr.toJSON(message.local_addr)
        : undefined);
    message.configure_update !== undefined &&
      (obj.configure_update = message.configure_update
        ? ConfigUpdate.toJSON(message.configure_update)
        : undefined);
    message.register_pk !== undefined &&
      (obj.register_pk = message.register_pk
        ? RegisterPk.toJSON(message.register_pk)
        : undefined);
    message.register_pk_response !== undefined &&
      (obj.register_pk_response = message.register_pk_response
        ? RegisterPkResponse.toJSON(message.register_pk_response)
        : undefined);
    message.software_update !== undefined &&
      (obj.software_update = message.software_update
        ? SoftwareUpdate.toJSON(message.software_update)
        : undefined);
    message.request_relay !== undefined &&
      (obj.request_relay = message.request_relay
        ? RequestRelay.toJSON(message.request_relay)
        : undefined);
    message.relay_response !== undefined &&
      (obj.relay_response = message.relay_response
        ? RelayResponse.toJSON(message.relay_response)
        : undefined);
    message.test_nat_request !== undefined &&
      (obj.test_nat_request = message.test_nat_request
        ? TestNatRequest.toJSON(message.test_nat_request)
        : undefined);
    message.test_nat_response !== undefined &&
      (obj.test_nat_response = message.test_nat_response
        ? TestNatResponse.toJSON(message.test_nat_response)
        : undefined);
    message.peer_discovery !== undefined &&
      (obj.peer_discovery = message.peer_discovery
        ? PeerDiscovery.toJSON(message.peer_discovery)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RendezvousMessage>, I>>(
    object: I
  ): RendezvousMessage {
    const message = createBaseRendezvousMessage();
    message.register_peer =
      object.register_peer !== undefined && object.register_peer !== null
        ? RegisterPeer.fromPartial(object.register_peer)
        : undefined;
    message.register_peer_response =
      object.register_peer_response !== undefined &&
      object.register_peer_response !== null
        ? RegisterPeerResponse.fromPartial(object.register_peer_response)
        : undefined;
    message.punch_hole_request =
      object.punch_hole_request !== undefined &&
      object.punch_hole_request !== null
        ? PunchHoleRequest.fromPartial(object.punch_hole_request)
        : undefined;
    message.punch_hole =
      object.punch_hole !== undefined && object.punch_hole !== null
        ? PunchHole.fromPartial(object.punch_hole)
        : undefined;
    message.punch_hole_sent =
      object.punch_hole_sent !== undefined && object.punch_hole_sent !== null
        ? PunchHoleSent.fromPartial(object.punch_hole_sent)
        : undefined;
    message.punch_hole_response =
      object.punch_hole_response !== undefined &&
      object.punch_hole_response !== null
        ? PunchHoleResponse.fromPartial(object.punch_hole_response)
        : undefined;
    message.fetch_local_addr =
      object.fetch_local_addr !== undefined && object.fetch_local_addr !== null
        ? FetchLocalAddr.fromPartial(object.fetch_local_addr)
        : undefined;
    message.local_addr =
      object.local_addr !== undefined && object.local_addr !== null
        ? LocalAddr.fromPartial(object.local_addr)
        : undefined;
    message.configure_update =
      object.configure_update !== undefined && object.configure_update !== null
        ? ConfigUpdate.fromPartial(object.configure_update)
        : undefined;
    message.register_pk =
      object.register_pk !== undefined && object.register_pk !== null
        ? RegisterPk.fromPartial(object.register_pk)
        : undefined;
    message.register_pk_response =
      object.register_pk_response !== undefined &&
      object.register_pk_response !== null
        ? RegisterPkResponse.fromPartial(object.register_pk_response)
        : undefined;
    message.software_update =
      object.software_update !== undefined && object.software_update !== null
        ? SoftwareUpdate.fromPartial(object.software_update)
        : undefined;
    message.request_relay =
      object.request_relay !== undefined && object.request_relay !== null
        ? RequestRelay.fromPartial(object.request_relay)
        : undefined;
    message.relay_response =
      object.relay_response !== undefined && object.relay_response !== null
        ? RelayResponse.fromPartial(object.relay_response)
        : undefined;
    message.test_nat_request =
      object.test_nat_request !== undefined && object.test_nat_request !== null
        ? TestNatRequest.fromPartial(object.test_nat_request)
        : undefined;
    message.test_nat_response =
      object.test_nat_response !== undefined &&
      object.test_nat_response !== null
        ? TestNatResponse.fromPartial(object.test_nat_response)
        : undefined;
    message.peer_discovery =
      object.peer_discovery !== undefined && object.peer_discovery !== null
        ? PeerDiscovery.fromPartial(object.peer_discovery)
        : undefined;
    return message;
  },
};

declare var self: any | undefined;
declare var window: any | undefined;
declare var global: any | undefined;
var globalThis: any = (() => {
  if (typeof globalThis !== "undefined") return globalThis;
  if (typeof self !== "undefined") return self;
  if (typeof window !== "undefined") return window;
  if (typeof global !== "undefined") return global;
  throw "Unable to locate global object";
})();

const atob: (b64: string) => string =
  globalThis.atob ||
  ((b64) => globalThis.Buffer.from(b64, "base64").toString("binary"));
function bytesFromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const arr = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; ++i) {
    arr[i] = bin.charCodeAt(i);
  }
  return arr;
}

const btoa: (bin: string) => string =
  globalThis.btoa ||
  ((bin) => globalThis.Buffer.from(bin, "binary").toString("base64"));
function base64FromBytes(arr: Uint8Array): string {
  const bin: string[] = [];
  for (const byte of arr) {
    bin.push(String.fromCharCode(byte));
  }
  return btoa(bin.join(""));
}

type Builtin =
  | Date
  | Function
  | Uint8Array
  | string
  | number
  | boolean
  | undefined;

export type DeepPartial<T> = T extends Builtin
  ? T
  : T extends Array<infer U>
  ? Array<DeepPartial<U>>
  : T extends ReadonlyArray<infer U>
  ? ReadonlyArray<DeepPartial<U>>
  : T extends {}
  ? { [K in keyof T]?: DeepPartial<T[K]> }
  : Partial<T>;

type KeysOfUnion<T> = T extends T ? keyof T : never;
export type Exact<P, I extends P> = P extends Builtin
  ? P
  : P & { [K in keyof P]: Exact<P[K], I[K]> } & Record<
        Exclude<keyof I, KeysOfUnion<P>>,
        never
      >;

if (_m0.util.Long !== Long) {
  _m0.util.Long = Long as any;
  _m0.configure();
}

function isSet(value: any): boolean {
  return value !== null && value !== undefined;
}
