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
  requestPk: boolean;
}

export interface PunchHoleRequest {
  id: string;
  natType: NatType;
  licenceKey: string;
  connType: ConnType;
}

export interface PunchHole {
  socketAddr: Uint8Array;
  relayServer: string;
  natType: NatType;
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
  socketAddr: Uint8Array;
  id: string;
  relayServer: string;
  natType: NatType;
  version: string;
}

export interface RegisterPk {
  id: string;
  uuid: Uint8Array;
  pk: Uint8Array;
  oldId: string;
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
  socketAddr: Uint8Array;
  pk: Uint8Array;
  failure: PunchHoleResponse_Failure;
  relayServer: string;
  natType: NatType | undefined;
  isLocal: boolean | undefined;
  otherFailure: string;
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
  rendezvousServers: string[];
}

export interface RequestRelay {
  id: string;
  uuid: string;
  socketAddr: Uint8Array;
  relayServer: string;
  secure: boolean;
  licenceKey: string;
  connType: ConnType;
}

export interface RelayResponse {
  socketAddr: Uint8Array;
  uuid: string;
  relayServer: string;
  id: string | undefined;
  pk: Uint8Array | undefined;
  refuseReason: string;
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
  socketAddr: Uint8Array;
  relayServer: string;
}

export interface LocalAddr {
  socketAddr: Uint8Array;
  localAddr: Uint8Array;
  relayServer: string;
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
  registerPeer: RegisterPeer | undefined;
  registerPeerResponse: RegisterPeerResponse | undefined;
  punchHoleRequest: PunchHoleRequest | undefined;
  punchHole: PunchHole | undefined;
  punchHoleSent: PunchHoleSent | undefined;
  punchHoleResponse: PunchHoleResponse | undefined;
  fetchLocalAddr: FetchLocalAddr | undefined;
  localAddr: LocalAddr | undefined;
  configureUpdate: ConfigUpdate | undefined;
  registerPk: RegisterPk | undefined;
  registerPkResponse: RegisterPkResponse | undefined;
  softwareUpdate: SoftwareUpdate | undefined;
  requestRelay: RequestRelay | undefined;
  relayResponse: RelayResponse | undefined;
  testNatRequest: TestNatRequest | undefined;
  testNatResponse: TestNatResponse | undefined;
  peerDiscovery: PeerDiscovery | undefined;
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
  return { requestPk: false };
}

export const RegisterPeerResponse = {
  encode(
    message: RegisterPeerResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.requestPk === true) {
      writer.uint32(16).bool(message.requestPk);
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
          message.requestPk = reader.bool();
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
      requestPk: isSet(object.requestPk) ? Boolean(object.requestPk) : false,
    };
  },

  toJSON(message: RegisterPeerResponse): unknown {
    const obj: any = {};
    message.requestPk !== undefined && (obj.requestPk = message.requestPk);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RegisterPeerResponse>, I>>(
    object: I
  ): RegisterPeerResponse {
    const message = createBaseRegisterPeerResponse();
    message.requestPk = object.requestPk ?? false;
    return message;
  },
};

function createBasePunchHoleRequest(): PunchHoleRequest {
  return { id: "", natType: 0, licenceKey: "", connType: 0 };
}

export const PunchHoleRequest = {
  encode(
    message: PunchHoleRequest,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== "") {
      writer.uint32(10).string(message.id);
    }
    if (message.natType !== 0) {
      writer.uint32(16).int32(message.natType);
    }
    if (message.licenceKey !== "") {
      writer.uint32(26).string(message.licenceKey);
    }
    if (message.connType !== 0) {
      writer.uint32(32).int32(message.connType);
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
          message.natType = reader.int32() as any;
          break;
        case 3:
          message.licenceKey = reader.string();
          break;
        case 4:
          message.connType = reader.int32() as any;
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
      natType: isSet(object.natType) ? natTypeFromJSON(object.natType) : 0,
      licenceKey: isSet(object.licenceKey) ? String(object.licenceKey) : "",
      connType: isSet(object.connType) ? connTypeFromJSON(object.connType) : 0,
    };
  },

  toJSON(message: PunchHoleRequest): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = message.id);
    message.natType !== undefined &&
      (obj.natType = natTypeToJSON(message.natType));
    message.licenceKey !== undefined && (obj.licenceKey = message.licenceKey);
    message.connType !== undefined &&
      (obj.connType = connTypeToJSON(message.connType));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PunchHoleRequest>, I>>(
    object: I
  ): PunchHoleRequest {
    const message = createBasePunchHoleRequest();
    message.id = object.id ?? "";
    message.natType = object.natType ?? 0;
    message.licenceKey = object.licenceKey ?? "";
    message.connType = object.connType ?? 0;
    return message;
  },
};

function createBasePunchHole(): PunchHole {
  return { socketAddr: new Uint8Array(), relayServer: "", natType: 0 };
}

export const PunchHole = {
  encode(
    message: PunchHole,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socketAddr.length !== 0) {
      writer.uint32(10).bytes(message.socketAddr);
    }
    if (message.relayServer !== "") {
      writer.uint32(18).string(message.relayServer);
    }
    if (message.natType !== 0) {
      writer.uint32(24).int32(message.natType);
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
          message.socketAddr = reader.bytes();
          break;
        case 2:
          message.relayServer = reader.string();
          break;
        case 3:
          message.natType = reader.int32() as any;
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
      socketAddr: isSet(object.socketAddr)
        ? bytesFromBase64(object.socketAddr)
        : new Uint8Array(),
      relayServer: isSet(object.relayServer) ? String(object.relayServer) : "",
      natType: isSet(object.natType) ? natTypeFromJSON(object.natType) : 0,
    };
  },

  toJSON(message: PunchHole): unknown {
    const obj: any = {};
    message.socketAddr !== undefined &&
      (obj.socketAddr = base64FromBytes(
        message.socketAddr !== undefined ? message.socketAddr : new Uint8Array()
      ));
    message.relayServer !== undefined &&
      (obj.relayServer = message.relayServer);
    message.natType !== undefined &&
      (obj.natType = natTypeToJSON(message.natType));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PunchHole>, I>>(
    object: I
  ): PunchHole {
    const message = createBasePunchHole();
    message.socketAddr = object.socketAddr ?? new Uint8Array();
    message.relayServer = object.relayServer ?? "";
    message.natType = object.natType ?? 0;
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
    socketAddr: new Uint8Array(),
    id: "",
    relayServer: "",
    natType: 0,
    version: "",
  };
}

export const PunchHoleSent = {
  encode(
    message: PunchHoleSent,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socketAddr.length !== 0) {
      writer.uint32(10).bytes(message.socketAddr);
    }
    if (message.id !== "") {
      writer.uint32(18).string(message.id);
    }
    if (message.relayServer !== "") {
      writer.uint32(26).string(message.relayServer);
    }
    if (message.natType !== 0) {
      writer.uint32(32).int32(message.natType);
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
          message.socketAddr = reader.bytes();
          break;
        case 2:
          message.id = reader.string();
          break;
        case 3:
          message.relayServer = reader.string();
          break;
        case 4:
          message.natType = reader.int32() as any;
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
      socketAddr: isSet(object.socketAddr)
        ? bytesFromBase64(object.socketAddr)
        : new Uint8Array(),
      id: isSet(object.id) ? String(object.id) : "",
      relayServer: isSet(object.relayServer) ? String(object.relayServer) : "",
      natType: isSet(object.natType) ? natTypeFromJSON(object.natType) : 0,
      version: isSet(object.version) ? String(object.version) : "",
    };
  },

  toJSON(message: PunchHoleSent): unknown {
    const obj: any = {};
    message.socketAddr !== undefined &&
      (obj.socketAddr = base64FromBytes(
        message.socketAddr !== undefined ? message.socketAddr : new Uint8Array()
      ));
    message.id !== undefined && (obj.id = message.id);
    message.relayServer !== undefined &&
      (obj.relayServer = message.relayServer);
    message.natType !== undefined &&
      (obj.natType = natTypeToJSON(message.natType));
    message.version !== undefined && (obj.version = message.version);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PunchHoleSent>, I>>(
    object: I
  ): PunchHoleSent {
    const message = createBasePunchHoleSent();
    message.socketAddr = object.socketAddr ?? new Uint8Array();
    message.id = object.id ?? "";
    message.relayServer = object.relayServer ?? "";
    message.natType = object.natType ?? 0;
    message.version = object.version ?? "";
    return message;
  },
};

function createBaseRegisterPk(): RegisterPk {
  return { id: "", uuid: new Uint8Array(), pk: new Uint8Array(), oldId: "" };
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
    if (message.oldId !== "") {
      writer.uint32(34).string(message.oldId);
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
          message.oldId = reader.string();
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
      oldId: isSet(object.oldId) ? String(object.oldId) : "",
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
    message.oldId !== undefined && (obj.oldId = message.oldId);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RegisterPk>, I>>(
    object: I
  ): RegisterPk {
    const message = createBaseRegisterPk();
    message.id = object.id ?? "";
    message.uuid = object.uuid ?? new Uint8Array();
    message.pk = object.pk ?? new Uint8Array();
    message.oldId = object.oldId ?? "";
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
    socketAddr: new Uint8Array(),
    pk: new Uint8Array(),
    failure: 0,
    relayServer: "",
    natType: undefined,
    isLocal: undefined,
    otherFailure: "",
  };
}

export const PunchHoleResponse = {
  encode(
    message: PunchHoleResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socketAddr.length !== 0) {
      writer.uint32(10).bytes(message.socketAddr);
    }
    if (message.pk.length !== 0) {
      writer.uint32(18).bytes(message.pk);
    }
    if (message.failure !== 0) {
      writer.uint32(24).int32(message.failure);
    }
    if (message.relayServer !== "") {
      writer.uint32(34).string(message.relayServer);
    }
    if (message.natType !== undefined) {
      writer.uint32(40).int32(message.natType);
    }
    if (message.isLocal !== undefined) {
      writer.uint32(48).bool(message.isLocal);
    }
    if (message.otherFailure !== "") {
      writer.uint32(58).string(message.otherFailure);
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
          message.socketAddr = reader.bytes();
          break;
        case 2:
          message.pk = reader.bytes();
          break;
        case 3:
          message.failure = reader.int32() as any;
          break;
        case 4:
          message.relayServer = reader.string();
          break;
        case 5:
          message.natType = reader.int32() as any;
          break;
        case 6:
          message.isLocal = reader.bool();
          break;
        case 7:
          message.otherFailure = reader.string();
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
      socketAddr: isSet(object.socketAddr)
        ? bytesFromBase64(object.socketAddr)
        : new Uint8Array(),
      pk: isSet(object.pk) ? bytesFromBase64(object.pk) : new Uint8Array(),
      failure: isSet(object.failure)
        ? punchHoleResponse_FailureFromJSON(object.failure)
        : 0,
      relayServer: isSet(object.relayServer) ? String(object.relayServer) : "",
      natType: isSet(object.natType)
        ? natTypeFromJSON(object.natType)
        : undefined,
      isLocal: isSet(object.isLocal) ? Boolean(object.isLocal) : undefined,
      otherFailure: isSet(object.otherFailure)
        ? String(object.otherFailure)
        : "",
    };
  },

  toJSON(message: PunchHoleResponse): unknown {
    const obj: any = {};
    message.socketAddr !== undefined &&
      (obj.socketAddr = base64FromBytes(
        message.socketAddr !== undefined ? message.socketAddr : new Uint8Array()
      ));
    message.pk !== undefined &&
      (obj.pk = base64FromBytes(
        message.pk !== undefined ? message.pk : new Uint8Array()
      ));
    message.failure !== undefined &&
      (obj.failure = punchHoleResponse_FailureToJSON(message.failure));
    message.relayServer !== undefined &&
      (obj.relayServer = message.relayServer);
    message.natType !== undefined &&
      (obj.natType =
        message.natType !== undefined
          ? natTypeToJSON(message.natType)
          : undefined);
    message.isLocal !== undefined && (obj.isLocal = message.isLocal);
    message.otherFailure !== undefined &&
      (obj.otherFailure = message.otherFailure);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PunchHoleResponse>, I>>(
    object: I
  ): PunchHoleResponse {
    const message = createBasePunchHoleResponse();
    message.socketAddr = object.socketAddr ?? new Uint8Array();
    message.pk = object.pk ?? new Uint8Array();
    message.failure = object.failure ?? 0;
    message.relayServer = object.relayServer ?? "";
    message.natType = object.natType ?? undefined;
    message.isLocal = object.isLocal ?? undefined;
    message.otherFailure = object.otherFailure ?? "";
    return message;
  },
};

function createBaseConfigUpdate(): ConfigUpdate {
  return { serial: 0, rendezvousServers: [] };
}

export const ConfigUpdate = {
  encode(
    message: ConfigUpdate,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.serial !== 0) {
      writer.uint32(8).int32(message.serial);
    }
    for (const v of message.rendezvousServers) {
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
          message.rendezvousServers.push(reader.string());
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
      rendezvousServers: Array.isArray(object?.rendezvousServers)
        ? object.rendezvousServers.map((e: any) => String(e))
        : [],
    };
  },

  toJSON(message: ConfigUpdate): unknown {
    const obj: any = {};
    message.serial !== undefined && (obj.serial = Math.round(message.serial));
    if (message.rendezvousServers) {
      obj.rendezvousServers = message.rendezvousServers.map((e) => e);
    } else {
      obj.rendezvousServers = [];
    }
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<ConfigUpdate>, I>>(
    object: I
  ): ConfigUpdate {
    const message = createBaseConfigUpdate();
    message.serial = object.serial ?? 0;
    message.rendezvousServers = object.rendezvousServers?.map((e) => e) || [];
    return message;
  },
};

function createBaseRequestRelay(): RequestRelay {
  return {
    id: "",
    uuid: "",
    socketAddr: new Uint8Array(),
    relayServer: "",
    secure: false,
    licenceKey: "",
    connType: 0,
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
    if (message.socketAddr.length !== 0) {
      writer.uint32(26).bytes(message.socketAddr);
    }
    if (message.relayServer !== "") {
      writer.uint32(34).string(message.relayServer);
    }
    if (message.secure === true) {
      writer.uint32(40).bool(message.secure);
    }
    if (message.licenceKey !== "") {
      writer.uint32(50).string(message.licenceKey);
    }
    if (message.connType !== 0) {
      writer.uint32(56).int32(message.connType);
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
          message.socketAddr = reader.bytes();
          break;
        case 4:
          message.relayServer = reader.string();
          break;
        case 5:
          message.secure = reader.bool();
          break;
        case 6:
          message.licenceKey = reader.string();
          break;
        case 7:
          message.connType = reader.int32() as any;
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
      socketAddr: isSet(object.socketAddr)
        ? bytesFromBase64(object.socketAddr)
        : new Uint8Array(),
      relayServer: isSet(object.relayServer) ? String(object.relayServer) : "",
      secure: isSet(object.secure) ? Boolean(object.secure) : false,
      licenceKey: isSet(object.licenceKey) ? String(object.licenceKey) : "",
      connType: isSet(object.connType) ? connTypeFromJSON(object.connType) : 0,
    };
  },

  toJSON(message: RequestRelay): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = message.id);
    message.uuid !== undefined && (obj.uuid = message.uuid);
    message.socketAddr !== undefined &&
      (obj.socketAddr = base64FromBytes(
        message.socketAddr !== undefined ? message.socketAddr : new Uint8Array()
      ));
    message.relayServer !== undefined &&
      (obj.relayServer = message.relayServer);
    message.secure !== undefined && (obj.secure = message.secure);
    message.licenceKey !== undefined && (obj.licenceKey = message.licenceKey);
    message.connType !== undefined &&
      (obj.connType = connTypeToJSON(message.connType));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RequestRelay>, I>>(
    object: I
  ): RequestRelay {
    const message = createBaseRequestRelay();
    message.id = object.id ?? "";
    message.uuid = object.uuid ?? "";
    message.socketAddr = object.socketAddr ?? new Uint8Array();
    message.relayServer = object.relayServer ?? "";
    message.secure = object.secure ?? false;
    message.licenceKey = object.licenceKey ?? "";
    message.connType = object.connType ?? 0;
    return message;
  },
};

function createBaseRelayResponse(): RelayResponse {
  return {
    socketAddr: new Uint8Array(),
    uuid: "",
    relayServer: "",
    id: undefined,
    pk: undefined,
    refuseReason: "",
    version: "",
  };
}

export const RelayResponse = {
  encode(
    message: RelayResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socketAddr.length !== 0) {
      writer.uint32(10).bytes(message.socketAddr);
    }
    if (message.uuid !== "") {
      writer.uint32(18).string(message.uuid);
    }
    if (message.relayServer !== "") {
      writer.uint32(26).string(message.relayServer);
    }
    if (message.id !== undefined) {
      writer.uint32(34).string(message.id);
    }
    if (message.pk !== undefined) {
      writer.uint32(42).bytes(message.pk);
    }
    if (message.refuseReason !== "") {
      writer.uint32(50).string(message.refuseReason);
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
          message.socketAddr = reader.bytes();
          break;
        case 2:
          message.uuid = reader.string();
          break;
        case 3:
          message.relayServer = reader.string();
          break;
        case 4:
          message.id = reader.string();
          break;
        case 5:
          message.pk = reader.bytes();
          break;
        case 6:
          message.refuseReason = reader.string();
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
      socketAddr: isSet(object.socketAddr)
        ? bytesFromBase64(object.socketAddr)
        : new Uint8Array(),
      uuid: isSet(object.uuid) ? String(object.uuid) : "",
      relayServer: isSet(object.relayServer) ? String(object.relayServer) : "",
      id: isSet(object.id) ? String(object.id) : undefined,
      pk: isSet(object.pk) ? bytesFromBase64(object.pk) : undefined,
      refuseReason: isSet(object.refuseReason)
        ? String(object.refuseReason)
        : "",
      version: isSet(object.version) ? String(object.version) : "",
    };
  },

  toJSON(message: RelayResponse): unknown {
    const obj: any = {};
    message.socketAddr !== undefined &&
      (obj.socketAddr = base64FromBytes(
        message.socketAddr !== undefined ? message.socketAddr : new Uint8Array()
      ));
    message.uuid !== undefined && (obj.uuid = message.uuid);
    message.relayServer !== undefined &&
      (obj.relayServer = message.relayServer);
    message.id !== undefined && (obj.id = message.id);
    message.pk !== undefined &&
      (obj.pk =
        message.pk !== undefined ? base64FromBytes(message.pk) : undefined);
    message.refuseReason !== undefined &&
      (obj.refuseReason = message.refuseReason);
    message.version !== undefined && (obj.version = message.version);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RelayResponse>, I>>(
    object: I
  ): RelayResponse {
    const message = createBaseRelayResponse();
    message.socketAddr = object.socketAddr ?? new Uint8Array();
    message.uuid = object.uuid ?? "";
    message.relayServer = object.relayServer ?? "";
    message.id = object.id ?? undefined;
    message.pk = object.pk ?? undefined;
    message.refuseReason = object.refuseReason ?? "";
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
  return { socketAddr: new Uint8Array(), relayServer: "" };
}

export const FetchLocalAddr = {
  encode(
    message: FetchLocalAddr,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socketAddr.length !== 0) {
      writer.uint32(10).bytes(message.socketAddr);
    }
    if (message.relayServer !== "") {
      writer.uint32(18).string(message.relayServer);
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
          message.socketAddr = reader.bytes();
          break;
        case 2:
          message.relayServer = reader.string();
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
      socketAddr: isSet(object.socketAddr)
        ? bytesFromBase64(object.socketAddr)
        : new Uint8Array(),
      relayServer: isSet(object.relayServer) ? String(object.relayServer) : "",
    };
  },

  toJSON(message: FetchLocalAddr): unknown {
    const obj: any = {};
    message.socketAddr !== undefined &&
      (obj.socketAddr = base64FromBytes(
        message.socketAddr !== undefined ? message.socketAddr : new Uint8Array()
      ));
    message.relayServer !== undefined &&
      (obj.relayServer = message.relayServer);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FetchLocalAddr>, I>>(
    object: I
  ): FetchLocalAddr {
    const message = createBaseFetchLocalAddr();
    message.socketAddr = object.socketAddr ?? new Uint8Array();
    message.relayServer = object.relayServer ?? "";
    return message;
  },
};

function createBaseLocalAddr(): LocalAddr {
  return {
    socketAddr: new Uint8Array(),
    localAddr: new Uint8Array(),
    relayServer: "",
    id: "",
    version: "",
  };
}

export const LocalAddr = {
  encode(
    message: LocalAddr,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.socketAddr.length !== 0) {
      writer.uint32(10).bytes(message.socketAddr);
    }
    if (message.localAddr.length !== 0) {
      writer.uint32(18).bytes(message.localAddr);
    }
    if (message.relayServer !== "") {
      writer.uint32(26).string(message.relayServer);
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
          message.socketAddr = reader.bytes();
          break;
        case 2:
          message.localAddr = reader.bytes();
          break;
        case 3:
          message.relayServer = reader.string();
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
      socketAddr: isSet(object.socketAddr)
        ? bytesFromBase64(object.socketAddr)
        : new Uint8Array(),
      localAddr: isSet(object.localAddr)
        ? bytesFromBase64(object.localAddr)
        : new Uint8Array(),
      relayServer: isSet(object.relayServer) ? String(object.relayServer) : "",
      id: isSet(object.id) ? String(object.id) : "",
      version: isSet(object.version) ? String(object.version) : "",
    };
  },

  toJSON(message: LocalAddr): unknown {
    const obj: any = {};
    message.socketAddr !== undefined &&
      (obj.socketAddr = base64FromBytes(
        message.socketAddr !== undefined ? message.socketAddr : new Uint8Array()
      ));
    message.localAddr !== undefined &&
      (obj.localAddr = base64FromBytes(
        message.localAddr !== undefined ? message.localAddr : new Uint8Array()
      ));
    message.relayServer !== undefined &&
      (obj.relayServer = message.relayServer);
    message.id !== undefined && (obj.id = message.id);
    message.version !== undefined && (obj.version = message.version);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<LocalAddr>, I>>(
    object: I
  ): LocalAddr {
    const message = createBaseLocalAddr();
    message.socketAddr = object.socketAddr ?? new Uint8Array();
    message.localAddr = object.localAddr ?? new Uint8Array();
    message.relayServer = object.relayServer ?? "";
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
    registerPeer: undefined,
    registerPeerResponse: undefined,
    punchHoleRequest: undefined,
    punchHole: undefined,
    punchHoleSent: undefined,
    punchHoleResponse: undefined,
    fetchLocalAddr: undefined,
    localAddr: undefined,
    configureUpdate: undefined,
    registerPk: undefined,
    registerPkResponse: undefined,
    softwareUpdate: undefined,
    requestRelay: undefined,
    relayResponse: undefined,
    testNatRequest: undefined,
    testNatResponse: undefined,
    peerDiscovery: undefined,
  };
}

export const RendezvousMessage = {
  encode(
    message: RendezvousMessage,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.registerPeer !== undefined) {
      RegisterPeer.encode(
        message.registerPeer,
        writer.uint32(50).fork()
      ).ldelim();
    }
    if (message.registerPeerResponse !== undefined) {
      RegisterPeerResponse.encode(
        message.registerPeerResponse,
        writer.uint32(58).fork()
      ).ldelim();
    }
    if (message.punchHoleRequest !== undefined) {
      PunchHoleRequest.encode(
        message.punchHoleRequest,
        writer.uint32(66).fork()
      ).ldelim();
    }
    if (message.punchHole !== undefined) {
      PunchHole.encode(message.punchHole, writer.uint32(74).fork()).ldelim();
    }
    if (message.punchHoleSent !== undefined) {
      PunchHoleSent.encode(
        message.punchHoleSent,
        writer.uint32(82).fork()
      ).ldelim();
    }
    if (message.punchHoleResponse !== undefined) {
      PunchHoleResponse.encode(
        message.punchHoleResponse,
        writer.uint32(90).fork()
      ).ldelim();
    }
    if (message.fetchLocalAddr !== undefined) {
      FetchLocalAddr.encode(
        message.fetchLocalAddr,
        writer.uint32(98).fork()
      ).ldelim();
    }
    if (message.localAddr !== undefined) {
      LocalAddr.encode(message.localAddr, writer.uint32(106).fork()).ldelim();
    }
    if (message.configureUpdate !== undefined) {
      ConfigUpdate.encode(
        message.configureUpdate,
        writer.uint32(114).fork()
      ).ldelim();
    }
    if (message.registerPk !== undefined) {
      RegisterPk.encode(message.registerPk, writer.uint32(122).fork()).ldelim();
    }
    if (message.registerPkResponse !== undefined) {
      RegisterPkResponse.encode(
        message.registerPkResponse,
        writer.uint32(130).fork()
      ).ldelim();
    }
    if (message.softwareUpdate !== undefined) {
      SoftwareUpdate.encode(
        message.softwareUpdate,
        writer.uint32(138).fork()
      ).ldelim();
    }
    if (message.requestRelay !== undefined) {
      RequestRelay.encode(
        message.requestRelay,
        writer.uint32(146).fork()
      ).ldelim();
    }
    if (message.relayResponse !== undefined) {
      RelayResponse.encode(
        message.relayResponse,
        writer.uint32(154).fork()
      ).ldelim();
    }
    if (message.testNatRequest !== undefined) {
      TestNatRequest.encode(
        message.testNatRequest,
        writer.uint32(162).fork()
      ).ldelim();
    }
    if (message.testNatResponse !== undefined) {
      TestNatResponse.encode(
        message.testNatResponse,
        writer.uint32(170).fork()
      ).ldelim();
    }
    if (message.peerDiscovery !== undefined) {
      PeerDiscovery.encode(
        message.peerDiscovery,
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
          message.registerPeer = RegisterPeer.decode(reader, reader.uint32());
          break;
        case 7:
          message.registerPeerResponse = RegisterPeerResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 8:
          message.punchHoleRequest = PunchHoleRequest.decode(
            reader,
            reader.uint32()
          );
          break;
        case 9:
          message.punchHole = PunchHole.decode(reader, reader.uint32());
          break;
        case 10:
          message.punchHoleSent = PunchHoleSent.decode(reader, reader.uint32());
          break;
        case 11:
          message.punchHoleResponse = PunchHoleResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 12:
          message.fetchLocalAddr = FetchLocalAddr.decode(
            reader,
            reader.uint32()
          );
          break;
        case 13:
          message.localAddr = LocalAddr.decode(reader, reader.uint32());
          break;
        case 14:
          message.configureUpdate = ConfigUpdate.decode(
            reader,
            reader.uint32()
          );
          break;
        case 15:
          message.registerPk = RegisterPk.decode(reader, reader.uint32());
          break;
        case 16:
          message.registerPkResponse = RegisterPkResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 17:
          message.softwareUpdate = SoftwareUpdate.decode(
            reader,
            reader.uint32()
          );
          break;
        case 18:
          message.requestRelay = RequestRelay.decode(reader, reader.uint32());
          break;
        case 19:
          message.relayResponse = RelayResponse.decode(reader, reader.uint32());
          break;
        case 20:
          message.testNatRequest = TestNatRequest.decode(
            reader,
            reader.uint32()
          );
          break;
        case 21:
          message.testNatResponse = TestNatResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 22:
          message.peerDiscovery = PeerDiscovery.decode(reader, reader.uint32());
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
      registerPeer: isSet(object.registerPeer)
        ? RegisterPeer.fromJSON(object.registerPeer)
        : undefined,
      registerPeerResponse: isSet(object.registerPeerResponse)
        ? RegisterPeerResponse.fromJSON(object.registerPeerResponse)
        : undefined,
      punchHoleRequest: isSet(object.punchHoleRequest)
        ? PunchHoleRequest.fromJSON(object.punchHoleRequest)
        : undefined,
      punchHole: isSet(object.punchHole)
        ? PunchHole.fromJSON(object.punchHole)
        : undefined,
      punchHoleSent: isSet(object.punchHoleSent)
        ? PunchHoleSent.fromJSON(object.punchHoleSent)
        : undefined,
      punchHoleResponse: isSet(object.punchHoleResponse)
        ? PunchHoleResponse.fromJSON(object.punchHoleResponse)
        : undefined,
      fetchLocalAddr: isSet(object.fetchLocalAddr)
        ? FetchLocalAddr.fromJSON(object.fetchLocalAddr)
        : undefined,
      localAddr: isSet(object.localAddr)
        ? LocalAddr.fromJSON(object.localAddr)
        : undefined,
      configureUpdate: isSet(object.configureUpdate)
        ? ConfigUpdate.fromJSON(object.configureUpdate)
        : undefined,
      registerPk: isSet(object.registerPk)
        ? RegisterPk.fromJSON(object.registerPk)
        : undefined,
      registerPkResponse: isSet(object.registerPkResponse)
        ? RegisterPkResponse.fromJSON(object.registerPkResponse)
        : undefined,
      softwareUpdate: isSet(object.softwareUpdate)
        ? SoftwareUpdate.fromJSON(object.softwareUpdate)
        : undefined,
      requestRelay: isSet(object.requestRelay)
        ? RequestRelay.fromJSON(object.requestRelay)
        : undefined,
      relayResponse: isSet(object.relayResponse)
        ? RelayResponse.fromJSON(object.relayResponse)
        : undefined,
      testNatRequest: isSet(object.testNatRequest)
        ? TestNatRequest.fromJSON(object.testNatRequest)
        : undefined,
      testNatResponse: isSet(object.testNatResponse)
        ? TestNatResponse.fromJSON(object.testNatResponse)
        : undefined,
      peerDiscovery: isSet(object.peerDiscovery)
        ? PeerDiscovery.fromJSON(object.peerDiscovery)
        : undefined,
    };
  },

  toJSON(message: RendezvousMessage): unknown {
    const obj: any = {};
    message.registerPeer !== undefined &&
      (obj.registerPeer = message.registerPeer
        ? RegisterPeer.toJSON(message.registerPeer)
        : undefined);
    message.registerPeerResponse !== undefined &&
      (obj.registerPeerResponse = message.registerPeerResponse
        ? RegisterPeerResponse.toJSON(message.registerPeerResponse)
        : undefined);
    message.punchHoleRequest !== undefined &&
      (obj.punchHoleRequest = message.punchHoleRequest
        ? PunchHoleRequest.toJSON(message.punchHoleRequest)
        : undefined);
    message.punchHole !== undefined &&
      (obj.punchHole = message.punchHole
        ? PunchHole.toJSON(message.punchHole)
        : undefined);
    message.punchHoleSent !== undefined &&
      (obj.punchHoleSent = message.punchHoleSent
        ? PunchHoleSent.toJSON(message.punchHoleSent)
        : undefined);
    message.punchHoleResponse !== undefined &&
      (obj.punchHoleResponse = message.punchHoleResponse
        ? PunchHoleResponse.toJSON(message.punchHoleResponse)
        : undefined);
    message.fetchLocalAddr !== undefined &&
      (obj.fetchLocalAddr = message.fetchLocalAddr
        ? FetchLocalAddr.toJSON(message.fetchLocalAddr)
        : undefined);
    message.localAddr !== undefined &&
      (obj.localAddr = message.localAddr
        ? LocalAddr.toJSON(message.localAddr)
        : undefined);
    message.configureUpdate !== undefined &&
      (obj.configureUpdate = message.configureUpdate
        ? ConfigUpdate.toJSON(message.configureUpdate)
        : undefined);
    message.registerPk !== undefined &&
      (obj.registerPk = message.registerPk
        ? RegisterPk.toJSON(message.registerPk)
        : undefined);
    message.registerPkResponse !== undefined &&
      (obj.registerPkResponse = message.registerPkResponse
        ? RegisterPkResponse.toJSON(message.registerPkResponse)
        : undefined);
    message.softwareUpdate !== undefined &&
      (obj.softwareUpdate = message.softwareUpdate
        ? SoftwareUpdate.toJSON(message.softwareUpdate)
        : undefined);
    message.requestRelay !== undefined &&
      (obj.requestRelay = message.requestRelay
        ? RequestRelay.toJSON(message.requestRelay)
        : undefined);
    message.relayResponse !== undefined &&
      (obj.relayResponse = message.relayResponse
        ? RelayResponse.toJSON(message.relayResponse)
        : undefined);
    message.testNatRequest !== undefined &&
      (obj.testNatRequest = message.testNatRequest
        ? TestNatRequest.toJSON(message.testNatRequest)
        : undefined);
    message.testNatResponse !== undefined &&
      (obj.testNatResponse = message.testNatResponse
        ? TestNatResponse.toJSON(message.testNatResponse)
        : undefined);
    message.peerDiscovery !== undefined &&
      (obj.peerDiscovery = message.peerDiscovery
        ? PeerDiscovery.toJSON(message.peerDiscovery)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RendezvousMessage>, I>>(
    object: I
  ): RendezvousMessage {
    const message = createBaseRendezvousMessage();
    message.registerPeer =
      object.registerPeer !== undefined && object.registerPeer !== null
        ? RegisterPeer.fromPartial(object.registerPeer)
        : undefined;
    message.registerPeerResponse =
      object.registerPeerResponse !== undefined &&
      object.registerPeerResponse !== null
        ? RegisterPeerResponse.fromPartial(object.registerPeerResponse)
        : undefined;
    message.punchHoleRequest =
      object.punchHoleRequest !== undefined && object.punchHoleRequest !== null
        ? PunchHoleRequest.fromPartial(object.punchHoleRequest)
        : undefined;
    message.punchHole =
      object.punchHole !== undefined && object.punchHole !== null
        ? PunchHole.fromPartial(object.punchHole)
        : undefined;
    message.punchHoleSent =
      object.punchHoleSent !== undefined && object.punchHoleSent !== null
        ? PunchHoleSent.fromPartial(object.punchHoleSent)
        : undefined;
    message.punchHoleResponse =
      object.punchHoleResponse !== undefined &&
      object.punchHoleResponse !== null
        ? PunchHoleResponse.fromPartial(object.punchHoleResponse)
        : undefined;
    message.fetchLocalAddr =
      object.fetchLocalAddr !== undefined && object.fetchLocalAddr !== null
        ? FetchLocalAddr.fromPartial(object.fetchLocalAddr)
        : undefined;
    message.localAddr =
      object.localAddr !== undefined && object.localAddr !== null
        ? LocalAddr.fromPartial(object.localAddr)
        : undefined;
    message.configureUpdate =
      object.configureUpdate !== undefined && object.configureUpdate !== null
        ? ConfigUpdate.fromPartial(object.configureUpdate)
        : undefined;
    message.registerPk =
      object.registerPk !== undefined && object.registerPk !== null
        ? RegisterPk.fromPartial(object.registerPk)
        : undefined;
    message.registerPkResponse =
      object.registerPkResponse !== undefined &&
      object.registerPkResponse !== null
        ? RegisterPkResponse.fromPartial(object.registerPkResponse)
        : undefined;
    message.softwareUpdate =
      object.softwareUpdate !== undefined && object.softwareUpdate !== null
        ? SoftwareUpdate.fromPartial(object.softwareUpdate)
        : undefined;
    message.requestRelay =
      object.requestRelay !== undefined && object.requestRelay !== null
        ? RequestRelay.fromPartial(object.requestRelay)
        : undefined;
    message.relayResponse =
      object.relayResponse !== undefined && object.relayResponse !== null
        ? RelayResponse.fromPartial(object.relayResponse)
        : undefined;
    message.testNatRequest =
      object.testNatRequest !== undefined && object.testNatRequest !== null
        ? TestNatRequest.fromPartial(object.testNatRequest)
        : undefined;
    message.testNatResponse =
      object.testNatResponse !== undefined && object.testNatResponse !== null
        ? TestNatResponse.fromPartial(object.testNatResponse)
        : undefined;
    message.peerDiscovery =
      object.peerDiscovery !== undefined && object.peerDiscovery !== null
        ? PeerDiscovery.fromPartial(object.peerDiscovery)
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
