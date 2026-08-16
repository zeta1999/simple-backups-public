package com.simpletools.backups

/**
 * Optional loader for `libbackups_ffi.so` (built from `crates/backups-ffi`).
 *
 * When the native library is absent, all methods return null / false and
 * [BackupNative] keeps using the pure-Kotlin stub.
 */
object NativeFfi {
    @Volatile
    var available: Boolean = false
        private set

    init {
        available = try {
            System.loadLibrary("backups_ffi")
            true
        } catch (_: UnsatisfiedLinkError) {
            false
        }
    }

    external fun nativeVersion(): String?
    external fun nativeParsePairPayload(payload: String): Array<String>?
    external fun nativeFormatPairPayload(addr: String, code: String): String?
    external fun nativePqcEnabled(): Boolean
    external fun nativeGeneratePairingCode(): String?
    external fun nativeIdentityEnsure(vault: String, password: String): String?
    external fun nativePair(
        vault: String,
        password: String,
        peer: String,
        addr: String,
        code: String,
        listen: Int,
    ): String?
    external fun nativeHasPeer(vault: String, password: String, peer: String): Boolean
    external fun nativeSnapshotAndPush(
        repo: String,
        source: String,
        vault: String,
        password: String,
        peer: String,
        addr: String,
        message: String,
    ): String?

    fun version(): String? = if (available) runCatching { nativeVersion() }.getOrNull() else null

    fun parsePairPayload(payload: String): Pair<String, String>? {
        if (!available) return null
        val parts = runCatching { nativeParsePairPayload(payload) }.getOrNull() ?: return null
        if (parts.size < 2) return null
        return parts[0] to parts[1]
    }

    fun formatPairPayload(addr: String, code: String): String? =
        if (available) runCatching { nativeFormatPairPayload(addr, code) }.getOrNull() else null

    fun pqcEnabled(): Boolean =
        available && runCatching { nativePqcEnabled() }.getOrDefault(false)

    fun generatePairingCode(): String? =
        if (available) runCatching { nativeGeneratePairingCode() }.getOrNull() else null

    fun identityEnsure(vault: String, password: String): String? =
        if (available) runCatching { nativeIdentityEnsure(vault, password) }.getOrNull() else null

    fun pair(
        vault: String,
        password: String,
        peer: String,
        addr: String,
        code: String,
        listen: Boolean,
    ): String? =
        if (available) {
            runCatching {
                nativePair(vault, password, peer, addr, code, if (listen) 1 else 0)
            }.getOrNull()
        } else {
            null
        }

    fun hasPeer(vault: String, password: String, peer: String): Boolean =
        available && runCatching { nativeHasPeer(vault, password, peer) }.getOrDefault(false)

    fun snapshotAndPush(
        repo: String,
        source: String,
        vault: String,
        password: String,
        peer: String,
        addr: String,
        message: String,
    ): String? =
        if (available) {
            runCatching {
                nativeSnapshotAndPush(repo, source, vault, password, peer, addr, message)
            }.getOrNull()
        } else {
            null
        }
}