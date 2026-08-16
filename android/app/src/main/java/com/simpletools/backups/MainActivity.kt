package com.simpletools.backups

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.text.InputType
import android.view.Menu
import android.view.MenuItem
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import org.json.JSONObject

class MainActivity : AppCompatActivity() {

    private lateinit var statusText: TextView
    private lateinit var fingerprintText: TextView
    private lateinit var btnScanPeer: Button
    private lateinit var btnConnect: Button
    private lateinit var btnToggleService: Button
    private lateinit var btnFullBackup: Button
    private lateinit var btnLiteBackup: Button
    private lateinit var btnResetLog: Button
    private lateinit var btnStopBackup: Button

    companion object {
        const val SCAN_REQUEST_PEER = "peer"
        const val EXTRA_SCAN_TYPE = "scan_type"
        const val EXTRA_SCAN_RESULT = "scan_result"
    }

    private val scanLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == RESULT_OK) {
            val connStr = result.data?.getStringExtra(EXTRA_SCAN_RESULT) ?: return@registerForActivityResult
            handleScanResult(connStr)
        }
    }

    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { _ -> refreshStatus() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        statusText = findViewById(R.id.status_text)
        fingerprintText = findViewById(R.id.fingerprint_text)
        btnScanPeer = findViewById(R.id.btn_scan_peer)
        btnConnect = findViewById(R.id.btn_connect)
        btnToggleService = findViewById(R.id.btn_toggle_service)
        btnFullBackup = findViewById(R.id.btn_full_backup)
        btnLiteBackup = findViewById(R.id.btn_lite_backup)
        btnResetLog = findViewById(R.id.btn_reset_log)
        btnStopBackup = findViewById(R.id.btn_stop_backup)

        BackupNative.init(this, filesDir.absolutePath)
        promptPassphrase()

        btnScanPeer.setOnClickListener { launchScanner() }
        btnScanPeer.setOnLongClickListener { showManualInput(); true }
        btnConnect.setOnClickListener { toggleConnect() }
        btnToggleService.setOnClickListener { toggleService() }

        btnFullBackup.setOnClickListener {
            AlertDialog.Builder(this)
                .setTitle("Push full snapshot")
                .setMessage("Push all local changes to the paired desktop repository (PQC channel).")
                .setPositiveButton("Start") { _, _ -> startFullBackup() }
                .setNegativeButton("Cancel", null)
                .show()
        }

        btnLiteBackup.setOnClickListener {
            AlertDialog.Builder(this)
                .setTitle("Push incremental")
                .setMessage("Push only files changed since the last sync.")
                .setPositiveButton("Start") { _, _ -> startLiteBackup() }
                .setNegativeButton("Cancel", null)
                .show()
        }

        btnResetLog.setOnClickListener {
            AlertDialog.Builder(this)
                .setTitle("Reset sync state")
                .setMessage("Clear local sync markers. Next incremental push re-checks everything.")
                .setPositiveButton("Reset") { _, _ -> resetLog() }
                .setNegativeButton("Cancel", null)
                .show()
        }

        btnStopBackup.setOnClickListener {
            BackupNative.stopBackup()
            Toast.makeText(this, "Stopping…", Toast.LENGTH_SHORT).show()
            statusText.postDelayed({ refreshStatus() }, 500)
        }

        requestPermissions()
        refreshStatus()
    }

    private fun promptPassphrase() {
        if (BackupNative.hasVault()) {
            showPassphraseDialog("Enter passphrase", "Unlock the local credentials vault:") { pass ->
                Thread {
                    try {
                        BackupNative.setPassphrase(pass)
                        BackupNative.loadIdentity()
                        runOnUiThread {
                            applySettings()
                            refreshStatus()
                        }
                    } catch (e: Exception) {
                        runOnUiThread {
                            Toast.makeText(this, "Wrong passphrase: ${e.message}", Toast.LENGTH_LONG).show()
                            promptPassphrase()
                        }
                    }
                }.start()
            }
        } else {
            showSetupDialog { nickname, pass ->
                Thread {
                    try {
                        BackupNative.setPassphrase(pass)
                        BackupNative.generateIdentity(nickname)
                        runOnUiThread {
                            Toast.makeText(this, "Identity created (stub bridge)", Toast.LENGTH_SHORT).show()
                            applySettings()
                            refreshStatus()
                        }
                    } catch (e: Exception) {
                        runOnUiThread {
                            Toast.makeText(this, "Setup failed: ${e.message}", Toast.LENGTH_LONG).show()
                        }
                    }
                }.start()
            }
        }
    }

    private fun showPassphraseDialog(title: String, message: String, onOk: (String) -> Unit) {
        val input = EditText(this).apply {
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            hint = "Passphrase"
        }
        AlertDialog.Builder(this)
            .setTitle(title)
            .setMessage(message)
            .setView(input)
            .setCancelable(false)
            .setPositiveButton("Unlock") { _, _ ->
                val text = input.text.toString()
                if (text.isNotEmpty()) onOk(text)
                else {
                    Toast.makeText(this, "Passphrase cannot be empty", Toast.LENGTH_SHORT).show()
                    promptPassphrase()
                }
            }
            .show()
    }

    private fun showSetupDialog(onOk: (String, String) -> Unit) {
        val layout = android.widget.LinearLayout(this).apply {
            orientation = android.widget.LinearLayout.VERTICAL
            setPadding(48, 16, 48, 0)
        }
        val nicknameInput = EditText(this).apply {
            hint = "Device nickname"
            setText("android-${Build.MODEL}")
        }
        val passInput = EditText(this).apply {
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            hint = "Set passphrase"
        }
        layout.addView(nicknameInput)
        layout.addView(passInput)

        AlertDialog.Builder(this)
            .setTitle("First-time setup")
            .setMessage("Choose a nickname and passphrase for the local vault.")
            .setView(layout)
            .setCancelable(false)
            .setPositiveButton("Create") { _, _ ->
                val nick = nicknameInput.text.toString().trim()
                val pass = passInput.text.toString()
                if (nick.isNotEmpty() && pass.isNotEmpty()) onOk(nick, pass)
                else {
                    Toast.makeText(this, "Both fields are required", Toast.LENGTH_SHORT).show()
                    showSetupDialog(onOk)
                }
            }
            .show()
    }

    private fun applySettings() {
        val prefs = getSharedPreferences("settings", MODE_PRIVATE)
        val peerAddr = prefs.getString("peer_addr", "") ?: ""
        if (peerAddr.isNotEmpty()) BackupNative.setPeerAddr(peerAddr)
        val watchDirs = prefs.getString("watch_dirs", "") ?: ""
        if (watchDirs.isNotEmpty()) BackupNative.setWatchDirs(watchDirs)
    }

    override fun onResume() {
        super.onResume()
        refreshStatus()
    }

    override fun onCreateOptionsMenu(menu: Menu): Boolean {
        menuInflater.inflate(R.menu.main_menu, menu)
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        return when (item.itemId) {
            R.id.action_settings -> {
                startActivity(Intent(this, SettingsActivity::class.java))
                true
            }
            else -> super.onOptionsItemSelected(item)
        }
    }

    private fun requestPermissions() {
        val needed = mutableListOf<String>()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_MEDIA_IMAGES)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.READ_MEDIA_IMAGES)
            }
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_MEDIA_VIDEO)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.READ_MEDIA_VIDEO)
            }
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA)
            != PackageManager.PERMISSION_GRANTED) {
            needed.add(Manifest.permission.CAMERA)
        }
        if (needed.isNotEmpty()) permissionLauncher.launch(needed.toTypedArray())
    }

    private fun showManualInput() {
        val input = EditText(this).apply {
            hint = "simple-backups:v1:pair:host:port:code"
            isSingleLine = true
        }
        AlertDialog.Builder(this)
            .setTitle("Manual pairing")
            .setMessage("Paste the desktop pairing payload (long-press to paste):")
            .setView(input)
            .setPositiveButton("Pair") { _, _ ->
                val text = input.text.toString().trim()
                if (text.isNotEmpty()) handleScanResult(text)
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun launchScanner() {
        val intent = Intent(this, ScannerActivity::class.java)
        intent.putExtra(EXTRA_SCAN_TYPE, SCAN_REQUEST_PEER)
        scanLauncher.launch(intent)
    }

    private fun handleScanResult(connStr: String) {
        Thread {
            try {
                BackupNative.pairWithPeer(connStr)
                runOnUiThread {
                    Toast.makeText(this, "Paired with desktop", Toast.LENGTH_SHORT).show()
                    refreshStatus()
                }
            } catch (e: Exception) {
                runOnUiThread {
                    Toast.makeText(this, "Pairing failed: ${e.message}", Toast.LENGTH_LONG).show()
                }
            }
        }.start()
    }

    private fun toggleConnect() {
        Thread {
            try {
                val json = JSONObject(BackupNative.getStatus())
                if (json.getBoolean("connected")) {
                    BackupNative.disconnect()
                    runOnUiThread {
                        Toast.makeText(this, "Disconnected", Toast.LENGTH_SHORT).show()
                        refreshStatus()
                    }
                } else {
                    BackupNative.connect()
                    runOnUiThread {
                        Toast.makeText(this, "Connected (stub)", Toast.LENGTH_SHORT).show()
                        refreshStatus()
                    }
                }
            } catch (e: Exception) {
                runOnUiThread {
                    Toast.makeText(this, e.message, Toast.LENGTH_LONG).show()
                }
            }
        }.start()
    }

    private fun toggleService() {
        val intent = Intent(this, BackupSyncService::class.java)
        if (BackupSyncService.isRunning) stopService(intent)
        else ContextCompat.startForegroundService(this, intent)
        statusText.postDelayed({ refreshStatus() }, 500)
    }

    private fun startFullBackup() {
        Thread {
            try {
                BackupNative.startFullBackup()
                runOnUiThread {
                    Toast.makeText(this, "Full push recorded (stub bridge)", Toast.LENGTH_SHORT).show()
                    refreshStatus()
                }
            } catch (e: Exception) {
                runOnUiThread {
                    Toast.makeText(this, "Backup failed: ${e.message}", Toast.LENGTH_LONG).show()
                }
            }
        }.start()
    }

    private fun startLiteBackup() {
        Thread {
            try {
                BackupNative.startLiteBackup()
                runOnUiThread {
                    Toast.makeText(this, "Incremental push recorded (stub bridge)", Toast.LENGTH_SHORT).show()
                    refreshStatus()
                }
            } catch (e: Exception) {
                runOnUiThread {
                    Toast.makeText(this, "Backup failed: ${e.message}", Toast.LENGTH_LONG).show()
                }
            }
        }.start()
    }

    private fun resetLog() {
        Thread {
            try {
                BackupNative.resetTransferLog()
                runOnUiThread {
                    Toast.makeText(this, "Sync state cleared", Toast.LENGTH_SHORT).show()
                    refreshStatus()
                }
            } catch (e: Exception) {
                runOnUiThread {
                    Toast.makeText(this, "Reset failed: ${e.message}", Toast.LENGTH_LONG).show()
                }
            }
        }.start()
    }

    private fun refreshStatus() {
        try {
            val json = JSONObject(BackupNative.getStatus())
            val lines = StringBuilder()
            lines.appendLine("bridge: ${json.optString("bridge", "?")}")
            lines.appendLine("identity: ${json.getBoolean("has_identity")}")
            lines.appendLine("vault: ${json.getBoolean("has_vault")}")
            lines.appendLine("paired: ${json.getBoolean("paired")}")
            lines.appendLine("connected: ${json.getBoolean("connected")}")
            lines.appendLine("watching: ${json.getBoolean("watching")}")
            lines.appendLine("backup running: ${json.getBoolean("backup_running")}")
            lines.appendLine("sync markers: ${json.getInt("log_count")}")
            lines.appendLine("peer: ${json.getString("peer_addr")}")
            statusText.text = lines.toString()

            val fp = BackupNative.getFingerprint()
            fingerprintText.text = if (fp.isNotEmpty()) "Fingerprint: $fp" else ""

            val paired = json.getBoolean("paired")
            val connected = json.getBoolean("connected")
            val backupActive = json.getBoolean("backup_running")

            btnConnect.isEnabled = paired
            btnConnect.text = if (connected) "Disconnect" else "Connect"
            btnToggleService.text =
                if (BackupSyncService.isRunning) "Stop watch service" else "Start watch service"
            btnToggleService.isEnabled = paired
            btnFullBackup.isEnabled = connected && !backupActive
            btnLiteBackup.isEnabled = connected && !backupActive
            btnResetLog.isEnabled = json.getBoolean("initialized")
            btnStopBackup.isEnabled = backupActive
        } catch (e: Exception) {
            statusText.text = "Status unavailable: ${e.message}"
        }
    }
}