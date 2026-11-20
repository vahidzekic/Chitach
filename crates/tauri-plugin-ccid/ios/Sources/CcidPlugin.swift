// USB smart-card readers for Chitach on iPhone and iPad (iOS 16+).
//
// iOS has built-in support for USB CCID readers and exposes them through
// CryptoTokenKit (TKSmartCardSlotManager). This plugin offers the same commands
// as the Android one: readers, requestPermission (nothing to ask on iOS),
// connect (ATR) and transmit (APDU), with hex strings for bytes.

import CryptoTokenKit
import Foundation
import SwiftRs
import Tauri
import UIKit
import WebKit

struct DeviceArgs: Decodable {
  let id: String
}

struct TransmitArgs: Decodable {
  let id: String
  let apdu: String
}

enum CcidError: Error, LocalizedError {
  case noReader
  case noCard
  case failed(String)

  var errorDescription: String? {
    switch self {
    case .noReader: return "no reader"
    case .noCard: return "no card"
    case .failed(let m): return m
    }
  }
}

private func toHex(_ data: Data) -> String {
  data.map { String(format: "%02X", $0) }.joined()
}

private func fromHex(_ s: String) throws -> Data {
  let chars = Array(s.trimmingCharacters(in: .whitespaces))
  guard chars.count % 2 == 0 else { throw CcidError.failed("bad hex") }
  var out = Data(capacity: chars.count / 2)
  var i = 0
  while i < chars.count {
    guard let b = UInt8(String(chars[i..<i + 2]), radix: 16) else {
      throw CcidError.failed("bad hex")
    }
    out.append(b)
    i += 2
  }
  return out
}

class CcidPlugin: Plugin {
  /// All CryptoTokenKit calls run here, one at a time.
  private let queue = DispatchQueue(label: "rs.chitach.ccid")
  /// Open card sessions by slot name.
  private var cards: [String: TKSmartCard] = [:]

  private func background(_ invoke: Invoke, _ body: @escaping () throws -> JsonObject) {
    queue.async {
      do {
        invoke.resolve(try body())
      } catch {
        invoke.reject(error.localizedDescription)
      }
    }
  }

  private func manager() throws -> TKSmartCardSlotManager {
    guard let m = TKSmartCardSlotManager.default else { throw CcidError.noReader }
    return m
  }

  private func slot(_ name: String) throws -> TKSmartCardSlot {
    let m = try manager()
    let sem = DispatchSemaphore(value: 0)
    var found: TKSmartCardSlot?
    m.getSlot(withName: name) { s in
      found = s
      sem.signal()
    }
    if sem.wait(timeout: .now() + 5) == .timedOut { throw CcidError.noReader }
    guard let s = found else { throw CcidError.noReader }
    return s
  }

  private func drop(_ name: String) {
    if let card = cards.removeValue(forKey: name) {
      card.endSession()
    }
  }

  @objc public func readers(_ invoke: Invoke) throws {
    background(invoke) {
      var list: [JsonObject] = []
      let names = (TKSmartCardSlotManager.default?.slotNames ?? []).sorted()
      for name in names {
        var present = false
        if let s = try? self.slot(name) {
          present = s.state == .validCard || s.state == .probing
        }
        if !present { self.drop(name) }
        list.append([
          "id": name,
          "label": name,
          "permission": true,
          "cardPresent": present,
          "tpduOnly": false,
        ])
      }
      for name in self.cards.keys where !names.contains(name) {
        self.drop(name)
      }
      return ["readers": list]
    }
  }

  @objc public func requestPermission(_ invoke: Invoke) throws {
    // iOS does not ask for permission to use a CCID reader.
    invoke.resolve()
  }

  @objc public func connect(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(DeviceArgs.self)
    background(invoke) {
      self.drop(args.id)
      let s = try self.slot(args.id)
      guard s.state == .validCard, let atr = s.atr else { throw CcidError.noCard }
      guard let card = s.makeSmartCard() else { throw CcidError.noCard }
      let sem = DispatchSemaphore(value: 0)
      var ok = false
      var err: Error?
      card.beginSession { success, error in
        ok = success
        err = error
        sem.signal()
      }
      if sem.wait(timeout: .now() + 20) == .timedOut { throw CcidError.failed("session timeout") }
      guard ok else { throw CcidError.failed(err?.localizedDescription ?? "cannot open card session") }
      self.cards[args.id] = card
      return ["hex": toHex(atr.bytes)]
    }
  }

  @objc public func transmit(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(TransmitArgs.self)
    background(invoke) {
      guard let card = self.cards[args.id], card.isValid else { throw CcidError.noCard }
      let apdu = try fromHex(args.apdu)
      let sem = DispatchSemaphore(value: 0)
      var rsp: Data?
      var err: Error?
      card.transmit(apdu) { data, error in
        rsp = data
        err = error
        sem.signal()
      }
      if sem.wait(timeout: .now() + 30) == .timedOut { throw CcidError.failed("transmit timeout") }
      guard let data = rsp else {
        // A failed transmit can leave the session unusable; start over on the next read.
        self.drop(args.id)
        throw CcidError.failed(err?.localizedDescription ?? "transmit failed")
      }
      return ["hex": toHex(data)]
    }
  }
}

@_cdecl("init_plugin_ccid")
public func initPlugin() -> Plugin {
  return CcidPlugin()
}
