// USB CCID smart-card readers for Chitach on Android.
//
// Android has no PC/SC service, so this plugin speaks the USB CCID protocol
// (the standard every PC/SC reader uses) directly over the USB host API:
// GetSlotStatus for card presence, IccPowerOn for the ATR and XfrBlock for
// APDUs. Readers that exchange whole APDUs (short or extended APDU level, which
// covers most current readers) are supported; TPDU-only readers are reported
// with tpduOnly=true so the app can tell the user.

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

    /** Reader only exchanges TPDUs (T=1 blocks are not implemented here). */
    val tpduOnly: Boolean
        get() = (features and 0x00070000L) == 0x00010000L

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
        return rsp.copyOfRange(HEADER, rsp.size)
    }

    fun transmit(apdu: ByteArray): ByteArray {
        val rsp = exchange(PC_TO_RDR_XFR_BLOCK, apdu, 0)
        checkStatus(rsp, "transmit")
        return rsp.copyOfRange(HEADER, rsp.size)
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
    private fun exchange(type: Byte, data: ByteArray, param: Byte): ByteArray {
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
