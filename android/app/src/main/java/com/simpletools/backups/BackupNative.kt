package com.simpletools.backups

import android.content.Context
import org.json.JSONObject
import java.io.File
import java.security.MessageDigest
import java.security.SecureRandom

/**
 * Mobile facade: uses [NativeFfi] (Rust PQC) when `libbackups_ffi.so` is present,
 * otherwise a pure-Kotlin stub for UI development.
 */
object BackupNative {
    private const val PREFS = "simple_backups_native"
    private const val PEER_NAME = "desktop"
    private var appCtx: Context? = null
    private var dataDir: File? = null
    private var unlocked = false
    private var passphrase: String? = null
    private var backupRunning = false

    fun init(context: Context, filesPath: String) {
        appCtx = context.applicationContext
        dataDir = File(filesPath, "simple-backups").also { it.mkdirs() }
    }

    private fun prefs() =
        appCtx!!.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    private fun vaultFile(): File = File(dataDir, "vault.bin")
    private fun repoDir(): File = File(dataDir, "repo")
    private fun stagingDir(): File = File(dataDir, "staging").also { it.mkdirs() }

    private fun useNativePqc(): Boolean = NativeFfi.available && NativeFfi.pqcEnabled()

    fun hasVault(): Boolean {
        if (useNativePqc() && vaultFile().exists()) return true
        return prefs().contains("pass_hash")
    }

    fun setPassphrase(pass: String) {
        if (useNativePqc() && vaultFile().exists()) {
            // Verify by opening vault.
            NativeFfi.identityEnsure(vaultFile().absolutePath, pass)
                ?: throw IllegalArgumentException("wrong passphrase or vault error")
            passphrase = pass
            unlocked = true
            return
        }
        val p = prefs()
        val existing = p.getString("pass_hash", null)
        val hash = sha256(pass)
        if (existing == null) {
            p.edit().putString("pass_hash", hash).apply()
        } else if (existing != hash) {
            throw IllegalArgumentException("wrong passphrase")
        }
        passphrase = pass
        unlocked = true
    }

    fun generateIdentity(nickname: String) {
        require(unlocked) { "vault locked" }
        val pass = passphrase ?: throw IllegalStateException("no passphrase")
        if (useNativePqc()) {
            val vk = NativeFfi.identityEnsure(vaultFile().absolutePath, pass)
                ?: throw IllegalStateException("native identity failed")
            prefs()
                .edit()
                .putString("nickname", nickname)
                .putString("identity_vk", vk)
                .putBoolean("has_identity", true)
                .apply()
            return
        }
        val seed = ByteArray(32).also { SecureRandom().nextBytes(it) }
        prefs()
            .edit()
            .putString("nickname", nickname)
            .putString("identity_seed", seed.joinToString("") { "%02x".format(it) })
            .putBoolean("has_identity", true)
            .apply()
    }

    fun loadIdentity() {
        require(unlocked) { "vault locked" }
        val pass = passphrase ?: throw IllegalStateException("no passphrase")
        if (useNativePqc()) {
            val vk = NativeFfi.identityEnsure(vaultFile().absolutePath, pass)
                ?: throw IllegalStateException("native identity failed")
            prefs().edit().putString("identity_vk", vk).putBoolean("has_identity", true).apply()
            return
        }
        if (!prefs().getBoolean("has_identity", false)) {
            throw IllegalStateException("no identity")
        }
    }

    fun getFingerprint(): String {
        val vk = prefs().getString("identity_vk", "") ?: ""
        if (vk.isNotEmpty()) return vk.take(16)
        val seed = prefs().getString("identity_seed", "") ?: return ""
        if (seed.isEmpty()) return ""
        return sha256(seed).take(16)
    }

    fun setPeerAddr(addr: String) {
        prefs().edit().putString("peer_addr", addr).apply()
    }

    fun setWatchDirs(dirs: String) {
        prefs().edit().putString("watch_dirs", dirs).apply()
    }

    fun pairWithPeer(payload: String) {
        require(unlocked) { "vault locked" }
        val parsed = parsePairPayload(payload)
        prefs()
            .edit()
            .putString("peer_addr", parsed.addr)
            .putString("pair_code", parsed.code)
            .apply()

        val pass = passphrase ?: throw IllegalStateException("no passphrase")
        if (useNativePqc() && parsed.code.isNotEmpty()) {
            // Phone initiates toward desktop listener.
            NativeFfi.identityEnsure(vaultFile().absolutePath, pass)
                ?: throw IllegalStateException("identity ensure failed")
            val peerVk = NativeFfi.pair(
                vaultFile().absolutePath,
                pass,
                PEER_NAME,
                parsed.addr,
                parsed.code,
                listen = false,
            ) ?: throw IllegalStateException("native pair failed")
            prefs()
                .edit()
                .putBoolean("paired", true)
                .putString("peer_vk", peerVk)
                .apply()
            return
        }
        prefs().edit().putBoolean("paired", true).apply()
    }

    fun connect() {
        require(prefs().getBoolean("paired", false)) { "not paired" }
        val pass = passphrase
        if (useNativePqc() && pass != null) {
            if (!NativeFfi.hasPeer(vaultFile().absolutePath, pass, PEER_NAME)) {
                throw IllegalStateException("peer not pinned in vault — pair again")
            }
        }
        prefs().edit().putBoolean("connected", true).apply()
    }

    fun disconnect() {
        prefs().edit().putBoolean("connected", false).apply()
        backupRunning = false
    }

    fun startFullBackup() {
        runPush("android-full") { editor ->
            editor.putLong("last_full_ms", System.currentTimeMillis())
        }
    }

    fun startLiteBackup() {
        // Object store is already incremental; lite shares the same push path for now.
        runPush("android-lite") { editor ->
            editor.putLong("last_lite_ms", System.currentTimeMillis())
        }
    }

    private fun runPush(message: String, stamp: (android.content.SharedPreferences.Editor) -> Unit) {
        require(prefs().getBoolean("connected", false)) { "not connected" }
        backupRunning = true
        try {
            val pass = passphrase
            val addr = prefs().getString("peer_addr", "") ?: ""
            if (useNativePqc() && pass != null && addr.isNotEmpty()) {
                val staging = stagingDir()
                File(staging, "mobile-heartbeat.txt").writeText(
                    "simple-backups mobile push\n${System.currentTimeMillis()}\n",
                )
                val result = NativeFfi.snapshotAndPush(
                    repoDir().absolutePath,
                    staging.absolutePath,
                    vaultFile().absolutePath,
                    pass,
                    PEER_NAME,
                    addr,
                    message,
                ) ?: throw IllegalStateException("native snapshot/push failed")
                prefs().edit().also {
                    it.putString("last_push", result)
                    stamp(it)
                    it.apply()
                }
            } else {
                prefs().edit().also {
                    stamp(it)
                    it.apply()
                }
            }
        } finally {
            backupRunning = false
        }
    }

    fun stopBackup() {
        backupRunning = false
    }

    fun resetTransferLog() {
        prefs().edit().remove("last_full_ms").remove("last_lite_ms").remove("last_push").apply()
    }

    fun getStatus(): String {
        val p = prefs()
        return JSONObject()
            .put("initialized", dataDir != null)
            .put("has_identity", p.getBoolean("has_identity", false))
            .put("has_vault", hasVault())
            .put("paired", p.getBoolean("paired", false))
            .put("connected", p.getBoolean("connected", false))
            .put("watching", BackupSyncService.isRunning)
            .put("backup_running", backupRunning)
            .put(
                "log_count",
                listOfNotNull(
                    p.getLong("last_full_ms", 0L).takeIf { it > 0 },
                    p.getLong("last_lite_ms", 0L).takeIf { it > 0 },
                ).size,
            )
            .put("peer_addr", p.getString("peer_addr", "") ?: "")
            .put("last_push", p.getString("last_push", "") ?: "")
            .put(
                "bridge",
                when {
                    useNativePqc() -> "native-pqc"
                    NativeFfi.available -> "native"
                    else -> "stub"
                },
            )
            .put("native_version", NativeFfi.version() ?: "")
            .toString()
    }

    data class PairPayload(val addr: String, val code: String)

    fun parsePairPayload(raw: String): PairPayload {
        NativeFfi.parsePairPayload(raw)?.let { (addr, code) ->
            return PairPayload(addr, code)
        }
        val s = raw.trim()
        if (s.startsWith("simple-backups:v1:pair:")) {
            val rest = s.removePrefix("simple-backups:v1:pair:")
            val idx = rest.lastIndexOf(':')
            require(idx > 0) { "invalid pair payload" }
            val code = rest.substring(idx + 1)
            val addr = rest.substring(0, idx)
            require(addr.isNotEmpty() && code.isNotEmpty()) { "invalid pair payload" }
            return PairPayload(addr, code)
        }
        return PairPayload(s, "")
    }

    private fun sha256(s: String): String {
        val d = MessageDigest.getInstance("SHA-256").digest(s.toByteArray())
        return d.joinToString("") { "%02x".format(it) }
    }
}