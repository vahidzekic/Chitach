// USB CCID smart-card readers for Chitach on Android.
//
// Android has no PC/SC service, so this plugin speaks the USB CCID protocol
// (the standard every PC/SC reader uses) directly over the USB host API:
// GetSlotStatus for card presence, IccPowerOn for the ATR and XfrBlock for
// APDUs. Readers that exchange whole APDUs (short or extended APDU level) get
// the APDU as is. TPDU-level readers (many cheap readers and combo SD + smart
// card readers) get the card protocol done here: SetParameters from the ATR,
// then T=1 blocks (ISO 7816-3 chapter 11) or T=0 command TPDUs. Only
// character-level readers are reported with tpduOnly=true (unsupported).

package rs.chitach.ccid

import android.app.Activity
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import android.os.Build
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import android.util.Log
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.util.concurrent.Executors

@InvokeArg
class DeviceArgs {
    lateinit var id: String
}

@InvokeArg
class TransmitArgs {
    lateinit var id: String
    lateinit var apdu: String
}

private const val ACTION_USB_PERMISSION = "rs.chitach.ccid.USB_PERMISSION"

// CCID message types (CCID specification rev 1.1, chapter 6).
private const val PC_TO_RDR_SET_PARAMETERS: Byte = 0x61
private const val PC_TO_RDR_ICC_POWER_ON: Byte = 0x62
private const val PC_TO_RDR_GET_SLOT_STATUS: Byte = 0x65
private const val PC_TO_RDR_XFR_BLOCK: Byte = 0x6F

private const val HEADER = 10
private const val TIMEOUT_MS = 20_000

private fun toHex(bytes: ByteArray): String {
    val sb = StringBuilder(bytes.size * 2)
    for (b in bytes) sb.append(String.format("%02X", b.toInt() and 0xFF))
    return sb.toString()
}

private fun fromHex(s: String): ByteArray {
    val clean = s.trim()
    require(clean.length % 2 == 0) { "bad hex" }
    return ByteArray(clean.length / 2) { i ->
        clean.substring(i * 2, i * 2 + 2).toInt(16).toByte()
    }
}

private fun le32(b: ByteArray, at: Int): Long =
    (b[at].toLong() and 0xFF) or
        ((b[at + 1].toLong() and 0xFF) shl 8) or
        ((b[at + 2].toLong() and 0xFF) shl 16) or
        ((b[at + 3].toLong() and 0xFF) shl 24)

private const val TAG = "ChitachCcid"

/** What the reader needs from the ATR to talk to the card at TPDU level. */
private class AtrInfo(
    /** Protocol used without PPS: TA2 in specific mode, otherwise the first one offered. */
    val protocol: Int,
    /** TA1 (Fi/Di), 0x11 when absent. */
    val ta1: Int,
    /** TA2 present: the card is in specific mode and runs at TA1 already. */
    val specificMode: Boolean,
    /** TC1 (extra guard time). */
    val tc1: Int,
    /** TC2 (T=0 waiting integer), default 10. */
    val wi: Int,
    /** First TA for T=1 (IFSC), default 32. */
    val ifsc: Int,
    /** First TB for T=1 (BWI/CWI), default BWI 4, CWI 13. */
    val bwiCwi: Int,
    /** First TC for T=1 bit 0: CRC instead of LRC. */
    val crc: Boolean,
) {
    companion object {
        fun parse(atr: ByteArray): AtrInfo {
            fun at(i: Int) = if (i < atr.size) atr[i].toInt() and 0xFF else 0
            var ta1 = 0x11
            var tc1 = 0
            var wi = 10
            var ifsc = 32
            var bwiCwi = 0x4D
            var crc = false
            var first = -1
            var specific = -1
            var pos = 1
            var y = at(pos) shr 4 // T0
            var level = 1
            var proto = 0 // protocol the current interface bytes belong to (level >= 3)
            var seenT1Ta = false
            var seenT1Tb = false
            var seenT1Tc = false
            while (pos < atr.size) {
                var ta = -1
                var tb = -1
                var tc = -1
                var td = -1
                if (y and 1 != 0) ta = at(++pos)
                if (y and 2 != 0) tb = at(++pos)
                if (y and 4 != 0) tc = at(++pos)
                if (y and 8 != 0) td = at(++pos)
                when (level) {
                    1 -> {
                        if (ta >= 0) ta1 = ta
                        if (tc >= 0) tc1 = tc
                    }
                    2 -> {
                        if (ta >= 0) specific = ta and 0x0F
                        if (tc >= 0) wi = tc
                    }
                    else -> if (proto == 1) {
                        if (ta >= 0 && !seenT1Ta) { ifsc = ta; seenT1Ta = true }
                        if (tb >= 0 && !seenT1Tb) { bwiCwi = tb; seenT1Tb = true }
                        if (tc >= 0 && !seenT1Tc) { crc = tc and 1 != 0; seenT1Tc = true }
                    }
                }
                if (td < 0) break
                proto = td and 0x0F
                if (first < 0) first = proto
                y = td shr 4
                level++
            }
            if (ifsc == 0 || ifsc == 0xFF) ifsc = 32
            val protocol = if (specific >= 0) specific else if (first < 0) 0 else first
            return AtrInfo(protocol, ta1, specific >= 0, tc1, wi, ifsc, bwiCwi, crc)
        }
    }
}

private fun ccidInterface(device: UsbDevice): UsbInterface? {
    for (i in 0 until device.interfaceCount) {
        val iface = device.getInterface(i)
        if (iface.interfaceClass == UsbConstants.USB_CLASS_CSCID) return iface
    }
    return null
}

/** One opened reader (slot 0 only, which is what card readers for ID cards have). */
private class CcidReader(
    val device: UsbDevice,
    private val conn: UsbDeviceConnection,
    private val iface: UsbInterface,
    private val epOut: UsbEndpoint,
    private val epIn: UsbEndpoint,
    /** dwFeatures from the CCID class descriptor, 0 when it could not be read. */
    private val features: Long,
) {
    private var seq = 0

    /** Exchange level from dwFeatures: 0 character, 1 TPDU, 2 short APDU, 4 extended APDU. */
    private val level: Int
        get() = ((features shr 16) and 0x07).toInt()

    /** The reader needs the card protocol done here (TPDU level). */
    private val tpdu: Boolean
        get() = level == 1

    /** Character-level readers are not supported (only reported, never used for reading). */
    val tpduOnly: Boolean
        get() = features != 0L && level == 0

    // TPDU state, reset on every power on.
    private var protocol = 0
    private var ifsc = 32
    private var ns = 0
    private var nr = 0

    fun close() {
        try {
            conn.releaseInterface(iface)
        } catch (_: Exception) {
        }
        conn.close()
    }

    fun cardPresent(): Boolean {
        val rsp = exchange(PC_TO_RDR_GET_SLOT_STATUS, ByteArray(0), 0)
        // bmICCStatus: 0 active, 1 present but inactive, 2 no card.
        return (rsp[7].toInt() and 0x03) != 2
    }

    fun powerOn(): ByteArray {
        // bPowerSelect 0 = automatic voltage selection.
        val rsp = exchange(PC_TO_RDR_ICC_POWER_ON, ByteArray(0), 0)
        checkStatus(rsp, "power on")
        val atr = rsp.copyOfRange(HEADER, rsp.size)
        if (tpdu) setupTpdu(atr)
        return atr
    }

    fun transmit(apdu: ByteArray): ByteArray {
        if (!tpdu) return xfr(apdu, 0, 0)
        return if (protocol == 1) transmitT1(apdu) else transmitT0(apdu)
    }

    private fun xfr(data: ByteArray, bwi: Int, levelParam: Int): ByteArray {
        val rsp = exchange(PC_TO_RDR_XFR_BLOCK, data, bwi.toByte(), levelParam)
        checkStatus(rsp, "transmit")
        return rsp.copyOfRange(HEADER, rsp.size)
    }

    // ---- TPDU level -------------------------------------------------------

    private fun setupTpdu(atr: ByteArray) {
        val info = AtrInfo.parse(atr)
        protocol = if (info.protocol == 1) 1 else 0
        ifsc = info.ifsc
        ns = 0
        nr = 0
        // Without PPS the card runs at the default rate unless it is in specific
        // mode; readers that do PPS themselves (dwFeatures 0x40 or 0x80) can use TA1.
        val autoPps = (features and 0xC0L) != 0L
        val fidi = if (info.specificMode || autoPps) info.ta1 else 0x11
        val params = if (protocol == 1) {
            byteArrayOf(
                fidi.toByte(),
                (0x10 or (if (info.crc) 1 else 0)).toByte(),
                info.tc1.toByte(),
                info.bwiCwi.toByte(),
                0, // clock stop not allowed
                info.ifsc.toByte(),
                0, // NAD
            )
        } else {
            byteArrayOf(fidi.toByte(), 0, info.tc1.toByte(), info.wi.toByte(), 0)
        }
        Log.i(TAG, String.format("TPDU reader: T=%d Fi/Di=%02X IFSC=%d features=%08X", protocol, fidi, ifsc, features))
        try {
            val rsp = exchange(PC_TO_RDR_SET_PARAMETERS, params, protocol.toByte())
            checkStatus(rsp, "set parameters")
        } catch (e: IOException) {
            // Some readers set the parameters from the ATR on their own and refuse this.
            Log.w(TAG, "SetParameters: ${e.message}")
        }
        if (protocol == 1) {
            if (info.crc) throw IOException("T=1 with CRC is not supported")
            // Tell the card we accept blocks of 254 bytes, unless the reader does it (0x400).
            if ((features and 0x400L) == 0L) {
                try {
                    val rsp = t1Transact(t1Block(0xC1, byteArrayOf(0xFE.toByte())))
                    if ((rsp[1].toInt() and 0xFF) != 0xE1) Log.w(TAG, "IFSD: unexpected answer")
                } catch (e: IOException) {
                    Log.w(TAG, "IFSD: ${e.message}")
                }
            }
        }
    }

    /** T=0: one command TPDU; 61xx / 6Cxx are handled by the caller (celik-core). */
    private fun transmitT0(apdu: ByteArray): ByteArray {
        if (apdu.size < 4) throw IOException("bad APDU")
        val cmd = when {
            apdu.size == 4 -> apdu + byteArrayOf(0)
            apdu.size == 5 -> apdu
            else -> {
                val lc = apdu[4].toInt() and 0xFF
                if (lc == 0) throw IOException("extended APDUs need a T=1 card")
                when (apdu.size) {
                    5 + lc -> apdu // case 3
                    6 + lc -> apdu.copyOfRange(0, 5 + lc) // case 4: the card answers 61xx
                    else -> throw IOException("bad APDU")
                }
            }
        }
        // The reader handles the T=0 procedure bytes (wLevelParameter is 0 at TPDU level).
        return xfr(cmd, 0, 0)
    }

    private fun t1Block(pcb: Int, inf: ByteArray): ByteArray {
        val b = ByteArray(4 + inf.size)
        b[0] = 0 // NAD
        b[1] = pcb.toByte()
        b[2] = inf.size.toByte()
        System.arraycopy(inf, 0, b, 3, inf.size)
        var lrc = 0
        for (i in 0 until b.size - 1) lrc = lrc xor (b[i].toInt() and 0xFF)
        b[b.size - 1] = lrc.toByte()
        return b
    }

    /** A well-formed block (length and LRC check out), or null. */
    private fun t1Parse(raw: ByteArray): ByteArray? {
        if (raw.size < 4) return null
        val len = raw[2].toInt() and 0xFF
        if (len == 0xFF || raw.size != 4 + len) return null
        var lrc = 0
        for (b in raw) lrc = lrc xor (b.toInt() and 0xFF)
        return if (lrc == 0) raw else null
    }

    /**
     * Sends one block and returns the card's answer, answering waiting time
     * extensions and IFS requests and asking again (R-block) after a bad block.
     */
    private fun t1Transact(first: ByteArray): ByteArray {
        var send = first
        var bwi = 0
        var errors = 0
        repeat(64) {
            val raw = try {
                xfr(send, bwi, 0)
            } catch (e: IOException) {
                if (e.message == "no card") throw e
                null
            }
            bwi = 0
            val blk = raw?.let { t1Parse(it) }
            if (blk == null) {
                if (++errors > 3) throw IOException("T=1: no valid answer from the card")
                // R-block with "EDC/parity error" asks the card to send its last block again.
                send = t1Block(0x81 or (nr shl 4), ByteArray(0))
                return@repeat
            }
            val pcb = blk[1].toInt() and 0xFF
            if (pcb and 0xC0 == 0xC0 && pcb and 0x20 == 0) {
                // S-block request from the card.
                when (pcb and 0x1F) {
                    0x03 -> { // WTX: keep waiting bwi times longer
                        bwi = if (blk.size > 4) blk[3].toInt() and 0xFF else 1
                        send = t1Block(0xE3, blk.copyOfRange(3, 3 + (blk[2].toInt() and 0xFF)))
                        return@repeat
                    }
                    0x01 -> { // IFS: card changes its block size
                        if (blk.size > 4) ifsc = (blk[3].toInt() and 0xFF).coerceIn(1, 254)
                        send = t1Block(0xE1, blk.copyOfRange(3, 3 + (blk[2].toInt() and 0xFF)))
                        return@repeat
                    }
                    else -> throw IOException(String.format("T=1: card sent S-block %02X", pcb))
                }
            }
            return blk
        }
        throw IOException("T=1: card keeps asking for more time")
    }

    private fun transmitT1(apdu: ByteArray): ByteArray {
        var rsp = ByteArray(0)
        var off = 0
        // Send, in chained I-blocks of at most IFSC bytes.
        while (true) {
            val n = minOf(ifsc, apdu.size - off)
            val more = off + n < apdu.size
            val block = t1Block((ns shl 6) or (if (more) 0x20 else 0), apdu.copyOfRange(off, off + n))
            var tries = 0
            while (true) {
                rsp = t1Transact(block)
                val pcb = rsp[1].toInt() and 0xFF
                if (pcb and 0xC0 == 0x80) {
                    // R-block: an ack for a chained block, or "send that again".
                    if (more && ((pcb shr 4) and 1) != ns) break
                    if (++tries > 3) throw IOException("T=1: card rejects the block")
                    continue
                }
                if (pcb and 0x80 == 0) {
                    if (more) throw IOException("T=1: card answered before the end of the command")
                    break
                }
                throw IOException(String.format("T=1: unexpected block %02X", pcb))
            }
            ns = ns xor 1
            off += n
            if (!more) break
        }
        // Receive: the answer may be chained too.
        val out = ByteArrayOutputStream()
        while (true) {
            val pcb = rsp[1].toInt() and 0xFF
            out.write(rsp, 3, rsp[2].toInt() and 0xFF)
            nr = ((pcb shr 6) and 1) xor 1
            if (pcb and 0x20 == 0) break
            val ack = t1Block(0x80 or (nr shl 4), ByteArray(0))
            var tries = 0
            while (true) {
                rsp = t1Transact(ack)
                val p = rsp[1].toInt() and 0xFF
                if (p and 0x80 == 0) break
                if (p and 0xC0 == 0x80 && ++tries <= 3) continue
                throw IOException(String.format("T=1: unexpected block %02X", p))
            }
        }
        return out.toByteArray()
    }

    private fun checkStatus(rsp: ByteArray, what: String) {
        val status = rsp[7].toInt() and 0xFF
        if ((status shr 6) and 0x03 == 1) {
            val error = rsp[8].toInt() and 0xFF
            if ((status and 0x03) == 2 || error == 0xFE) throw IOException("no card")
            throw IOException(String.format("reader error on %s: bError=%02X bStatus=%02X", what, error, status))
        }
    }

    @Synchronized
    private fun exchange(type: Byte, data: ByteArray, param: Byte, levelParam: Int = 0): ByteArray {
        val s = seq
        seq = (seq + 1) and 0xFF
        val msg = ByteArray(HEADER + data.size)
        msg[0] = type
        msg[1] = (data.size and 0xFF).toByte()
        msg[2] = ((data.size shr 8) and 0xFF).toByte()
        msg[3] = ((data.size shr 16) and 0xFF).toByte()
        msg[4] = ((data.size shr 24) and 0xFF).toByte()
        msg[5] = 0 // slot
        msg[6] = s.toByte()
        msg[7] = param
        msg[8] = (levelParam and 0xFF).toByte()
        msg[9] = ((levelParam shr 8) and 0xFF).toByte()
        System.arraycopy(data, 0, msg, HEADER, data.size)

        var off = 0
        while (off < msg.size) {
            val n = conn.bulkTransfer(epOut, msg, off, msg.size - off, TIMEOUT_MS)
            if (n < 0) throw IOException("USB write failed")
            off += n
        }

        // Skip stale answers (other sequence numbers) and time extension requests.
        repeat(200) {
            val rsp = readMessage()
            if ((rsp[6].toInt() and 0xFF) != s) return@repeat
            if (((rsp[7].toInt() and 0xFF) shr 6) == 2) return@repeat
            return rsp
        }
        throw IOException("reader did not answer")
    }

    private fun readMessage(): ByteArray {
        val chunk = ByteArray(16384)
        var n = conn.bulkTransfer(epIn, chunk, chunk.size, TIMEOUT_MS)
        if (n < HEADER) throw IOException("USB read failed")
        val len = le32(chunk, 1)
        if (len < 0 || len > 0x10000) throw IOException("bad CCID message length $len")
        val out = ByteArray(HEADER + len.toInt())
        var got = minOf(n, out.size)
        System.arraycopy(chunk, 0, out, 0, got)
        while (got < out.size) {
            n = conn.bulkTransfer(epIn, chunk, chunk.size, TIMEOUT_MS)
            if (n <= 0) throw IOException("USB read failed")
            val take = minOf(n, out.size - got)
            System.arraycopy(chunk, 0, out, got, take)
            got += take
        }
        return out
    }

    companion object {
        fun open(usb: UsbManager, device: UsbDevice): CcidReader? {
            val iface = ccidInterface(device) ?: return null
            var epOut: UsbEndpoint? = null
            var epIn: UsbEndpoint? = null
            for (i in 0 until iface.endpointCount) {
                val ep = iface.getEndpoint(i)
                if (ep.type != UsbConstants.USB_ENDPOINT_XFER_BULK) continue
                if (ep.direction == UsbConstants.USB_DIR_OUT) epOut = ep else epIn = ep
            }
            if (epOut == null || epIn == null) return null
            val conn = usb.openDevice(device) ?: return null
            if (!conn.claimInterface(iface, true)) {
                conn.close()
                return null
            }
            return CcidReader(device, conn, iface, epOut, epIn, readFeatures(conn.rawDescriptors))
        }

        /** dwFeatures of the CCID class descriptor (type 0x21) that follows the CCID interface. */
        private fun readFeatures(raw: ByteArray?): Long {
            if (raw == null) return 0
            var i = 0
            var inCcid = false
            while (i + 1 < raw.size) {
                val len = raw[i].toInt() and 0xFF
                if (len < 2) break
                val type = raw[i + 1].toInt() and 0xFF
                if (type == 0x04 && i + 5 < raw.size) {
                    inCcid = (raw[i + 5].toInt() and 0xFF) == UsbConstants.USB_CLASS_CSCID
                } else if (type == 0x21 && inCcid && len >= 54 && i + 44 <= raw.size) {
                    return le32(raw, i + 40)
                }
                i += len
            }
            return 0
        }
    }
}

@TauriPlugin
class CcidPlugin(private val activity: Activity) : Plugin(activity) {
    private val usb = activity.getSystemService(Context.USB_SERVICE) as UsbManager
    private val worker = Executors.newSingleThreadExecutor()
    private val opened = HashMap<String, CcidReader>()

    private fun background(invoke: Invoke, body: () -> JSObject) {
        worker.execute {
            try {
                invoke.resolve(body())
            } catch (e: Exception) {
                invoke.reject(e.message ?: e.toString())
            }
        }
    }

    private fun ccidDevices(): List<UsbDevice> =
        usb.deviceList.values.filter { ccidInterface(it) != null }.sortedBy { it.deviceName }

    private fun device(id: String): UsbDevice =
        usb.deviceList[id] ?: throw IOException("no reader")

    private fun reader(id: String): CcidReader {
        opened[id]?.let { return it }
        val dev = device(id)
        if (!usb.hasPermission(dev)) throw IOException("no USB permission")
        val r = CcidReader.open(usb, dev) ?: throw IOException("cannot open reader")
        opened[id] = r
        return r
    }

    private fun drop(id: String) {
        opened.remove(id)?.close()
    }

    private fun label(dev: UsbDevice): String {
        val parts = try {
            listOfNotNull(dev.manufacturerName, dev.productName)
        } catch (_: SecurityException) {
            emptyList()
        }
        return parts.joinToString(" ").trim().ifEmpty { "USB card reader" }
    }

    @Command
    fun readers(invoke: Invoke) = background(invoke) {
        val list = JSArray()
        val seen = HashSet<String>()
        val labels = HashMap<String, Int>()
        for (dev in ccidDevices()) {
            seen.add(dev.deviceName)
            val permission = usb.hasPermission(dev)
            var present = false
            var tpduOnly = false
            if (permission) {
                try {
                    val r = reader(dev.deviceName)
                    tpduOnly = r.tpduOnly
                    present = r.cardPresent()
                } catch (_: Exception) {
                    drop(dev.deviceName)
                }
            }
            var name = label(dev)
            val count = (labels[name] ?: 0) + 1
            labels[name] = count
            if (count > 1) name = "$name ($count)"
            val o = JSObject()
            o.put("id", dev.deviceName)
            o.put("label", name)
            o.put("permission", permission)
            o.put("cardPresent", present)
            o.put("tpduOnly", tpduOnly)
            list.put(o)
        }
        for (id in opened.keys.toList()) if (id !in seen) drop(id)
        val res = JSObject()
        res.put("readers", list)
        res
    }

    @Command
    fun requestPermission(invoke: Invoke) {
        val args = invoke.parseArgs(DeviceArgs::class.java)
        background(invoke) {
            val dev = device(args.id)
            if (!usb.hasPermission(dev)) {
                val intent = Intent(ACTION_USB_PERMISSION).setPackage(activity.packageName)
                val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
                usb.requestPermission(dev, PendingIntent.getBroadcast(activity, 0, intent, flags))
            }
            JSObject()
        }
    }

    @Command
    fun connect(invoke: Invoke) {
        val args = invoke.parseArgs(DeviceArgs::class.java)
        background(invoke) {
            val atr = try {
                reader(args.id).powerOn()
            } catch (e: IOException) {
                drop(args.id)
                throw e
            }
            val o = JSObject()
            o.put("hex", toHex(atr))
            o
        }
    }

    @Command
    fun transmit(invoke: Invoke) {
        val args = invoke.parseArgs(TransmitArgs::class.java)
        background(invoke) {
            val rsp = reader(args.id).transmit(fromHex(args.apdu))
            val o = JSObject()
            o.put("hex", toHex(rsp))
            o
        }
    }
}
